// generator.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides Rust bindings for the
//! [`ctranslate2::Generator`](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html).
//!
//! The [`Generator`] structure is the primary interface in this module, offering the capability
//! to generate text based on a trained model. It is designed for tasks such as text generation,
//! autocompletion, and other similar language generation tasks.
//!
//! Alongside the `Generator`, this module also includes structures that are critical for
//! controlling and understanding the generation process:
//!
//! - [`GenerationOptions`]: A structure containing configuration options for the generation
//!   process,
//!
//! - [`GenerationResult`]: A structure that holds the results of the generation process.
//!

use std::path::Path;

use anyhow::{anyhow, Error, Result};
use cxx::UniquePtr;

use crate::config::{BatchType, Config};
pub use crate::types::ffi::GenerationStepResult;
use crate::types::vec_ffi_vecstr;

trait GenerationCallback {
    fn execute(&mut self, res: GenerationStepResult) -> bool;
}

impl<F: FnMut(GenerationStepResult) -> bool> GenerationCallback for F {
    fn execute(&mut self, args: GenerationStepResult) -> bool {
        self(args)
    }
}

type GenerationCallbackBox<'a> = Box<dyn GenerationCallback + 'a>;

impl<'a> From<Option<&'a mut dyn FnMut(GenerationStepResult) -> bool>>
    for GenerationCallbackBox<'a>
{
    fn from(opt: Option<&'a mut dyn FnMut(GenerationStepResult) -> bool>) -> Self {
        match opt {
            None => Box::new(|_| false) as GenerationCallbackBox,
            Some(c) => Box::new(c) as GenerationCallbackBox,
        }
    }
}

fn execute_generation_callback(f: &mut GenerationCallbackBox, arg: GenerationStepResult) -> bool {
    f.execute(arg)
}

#[cxx::bridge]
mod ffi {
    struct GenerationOptions<'a> {
        beam_size: usize,
        patience: f32,
        length_penalty: f32,
        repetition_penalty: f32,
        no_repeat_ngram_size: usize,
        disable_unk: bool,
        suppress_sequences: Vec<VecStr<'a>>,
        return_end_token: bool,
        max_length: usize,
        min_length: usize,
        sampling_topk: usize,
        sampling_topp: f32,
        sampling_temperature: f32,
        num_hypotheses: usize,
        return_scores: bool,
        return_alternatives: bool,
        min_alternative_expansion_prob: f32,
        static_prompt: Vec<&'a str>,
        cache_static_prompt: bool,
        include_prompt_in_result: bool,
        max_batch_size: usize,
        batch_type: BatchType,
    }

    struct GenerationResult {
        sequences: Vec<VecString>,
        sequences_ids: Vec<VecUSize>,
        scores: Vec<f32>,
    }

    extern "Rust" {
        type GenerationCallbackBox<'a>;
        fn execute_generation_callback(
            f: &mut GenerationCallbackBox,
            arg: GenerationStepResult,
        ) -> bool;
    }

    unsafe extern "C++" {
        include!("ct2rs/src/types.rs.h");
        include!("ct2rs/include/generator.h");

        type VecString = crate::types::ffi::VecString;
        type VecStr<'a> = crate::types::ffi::VecStr<'a>;
        type VecUSize = crate::types::ffi::VecUSize;

        type Config = crate::config::ffi::Config;
        type BatchType = crate::config::ffi::BatchType;
        type GenerationStepResult = crate::types::ffi::GenerationStepResult;

        type Generator;

        fn generator(model_path: &str, config: UniquePtr<Config>) -> Result<UniquePtr<Generator>>;

        fn generate_batch(
            self: &Generator,
            start_tokens: &Vec<VecStr>,
            options: &GenerationOptions,
            has_callback: bool,
            callback: &mut GenerationCallbackBox,
        ) -> Result<Vec<GenerationResult>>;

        fn num_queued_batches(self: &Generator) -> Result<usize>;

        fn num_active_batches(self: &Generator) -> Result<usize>;

        fn num_replicas(self: &Generator) -> Result<usize>;
    }
}

unsafe impl Send for ffi::Generator {}
unsafe impl Sync for ffi::Generator {}

/// A Rust binding to the
/// [`ctranslate2::Generator`](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html).
///
/// # Example
///
/// ```no_run
/// # use anyhow::Result;
/// use ct2rs::config::{Config, Device};
/// use ct2rs::generator::{Generator, GenerationOptions};
///
/// # fn main() -> Result<()> {
/// let generator = Generator::new("/path/to/model", &Config::default())?;
/// let res = generator.generate_batch(
///     &vec![vec!["▁Hello", "▁world", "!", "</s>", "<unk>"]],
///     &GenerationOptions::default(),
///     None
/// )?;
/// for r in res {
///     println!("{:?}", r);
/// }
/// # Ok(())
/// # }
/// ```
pub struct Generator {
    ptr: UniquePtr<ffi::Generator>,
}

impl Generator {
    /// Creates and initializes an instance of `Generator`.
    ///
    /// This function constructs a new `Generator` by loading a language model from the specified
    /// `model_path` and applying the provided `config` settings.
    ///
    /// # Arguments
    /// * `model_path` - A path to the directory containing the language model to be loaded.
    /// * `config` - A reference to a `Config` structure that specifies various settings
    ///   and configurations for the `Generator`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Generator`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    ///
    /// # Example
    /// ```no_run
    /// use ct2rs::config::Config;
    /// use ct2rs::generator::Generator;
    ///
    /// let config = Config::default();
    /// let generator = Generator::new("/path/to/model", &config)
    ///     .expect("Failed to create generator");
    /// ```
    pub fn new<T: AsRef<Path>>(model_path: T, config: &Config) -> Result<Generator> {
        Ok(Generator {
            ptr: ffi::generator(
                model_path
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid path: {}", model_path.as_ref().display()))?,
                config.to_ffi(),
            )?,
        })
    }

    /// Generates a sequence of tokens following the provided batch of start tokens.
    ///
    /// This function generates tokens sequentially starting from the given `start_tokens`.
    /// If the decoding process should start with a specific start token such as `<s>`,
    /// it needs to be included in the input. The generation continues according to the
    /// options specified in `options`.
    ///
    /// An optional `callback` can be provided, which is called with each token generated
    /// during the process. This callback allows for monitoring and reacting to the generation
    /// step-by-step. If the callback returns `true`, the generation process for the current batch
    /// will be stopped. It's important to note that if a callback is used, `options.beam_size`
    /// must be set to `1`.
    ///
    /// # Arguments
    /// * `start_tokens` - A vector of vectors containing start tokens for each sequence in the
    ///   batch. These tokens represent the initial state of the generation process.
    /// * `options` - Settings applied to the generation process, such as beam size and other
    ///   generation-specific configurations.
    /// * `callback` - An optional mutable reference to a closure that is invoked for each
    ///   generation step. The closure takes a `GenerationStepResult` and returns a `bool`.
    ///   Returning `true` will stop the generation for that batch.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `GenerationResult` if successful, encapsulating
    /// the generated sequences for each input start token batch, or an error if the generation fails.
    ///
    /// # Example
    /// ```no_run
    /// use ct2rs::config::Config;
    /// use ct2rs::generator::{Generator, GenerationOptions, GenerationStepResult};
    ///
    /// let start_tokens = vec![vec!["<s>".to_string()]];
    /// let options = GenerationOptions::default();
    /// let mut callback = |step_result: GenerationStepResult| -> bool {
    ///     println!("{:?}", step_result);
    ///     false // Continue processing
    /// };
    /// let generator = Generator::new("/path/to/model", &Config::default())
    ///     .expect("Failed to create generator");
    /// let results = generator.generate_batch(&start_tokens, &options, Some(&mut callback))
    ///     .expect("Generation failed");
    /// ```
    pub fn generate_batch<'a, T: AsRef<str>, U: AsRef<str>, V: AsRef<str>>(
        &self,
        start_tokens: &Vec<Vec<T>>,
        options: &GenerationOptions<U, V>,
        callback: Option<&'a mut dyn FnMut(GenerationStepResult) -> bool>,
    ) -> Result<Vec<GenerationResult>> {
        Ok(self
            .ptr
            .generate_batch(
                &vec_ffi_vecstr(start_tokens),
                &options.to_ffi(),
                callback.is_some(),
                &mut GenerationCallbackBox::from(callback),
            )?
            .into_iter()
            .map(GenerationResult::from)
            .collect())
    }

    /// Number of batches in the work queue.
    #[inline]
    pub fn num_queued_batches(&self) -> Result<usize> {
        self.ptr.num_queued_batches().map_err(Error::from)
    }

    /// Number of batches in the work queue or currently processed by a worker.
    #[inline]
    pub fn num_active_batches(&self) -> Result<usize> {
        self.ptr.num_active_batches().map_err(Error::from)
    }

    /// Number of parallel replicas.
    #[inline]
    pub fn num_replicas(&self) -> Result<usize> {
        self.ptr.num_replicas().map_err(Error::from)
    }
}

/// The set of generation options.
#[derive(Clone, Debug)]
pub struct GenerationOptions<T: AsRef<str>, U: AsRef<str>> {
    /// Beam size to use for beam search (set 1 to run greedy search).
    pub beam_size: usize,
    /// Beam search patience factor, as described in <https://arxiv.org/abs/2204.05424>.
    /// The decoding will continue until beam_size*patience hypotheses are finished.
    pub patience: f32,
    /// Exponential penalty applied to the length during beam search.
    /// The scores are normalized with:
    /// ```math
    ///   hypothesis_score /= (hypothesis_length ** length_penalty)
    /// ```
    pub length_penalty: f32,
    /// Penalty applied to the score of previously generated tokens, as described in
    /// <https://arxiv.org/abs/1909.05858> (set > 1 to penalize).
    pub repetition_penalty: f32,
    /// Prevent repetitions of ngrams with this size (set 0 to disable).
    pub no_repeat_ngram_size: usize,
    /// Disable the generation of the unknown token.
    pub disable_unk: bool,
    /// Disable the generation of some sequences of tokens.
    pub suppress_sequences: Vec<Vec<T>>,
    // Stop the decoding on one of these tokens (defaults to the model EOS token).
    //std::variant<std::string, std::vector<std::string>, std::vector<size_t>> end_token;
    /// Include the end token in the result.
    pub return_end_token: bool,
    /// Length constraints.
    pub max_length: usize,
    /// Length constraints.
    pub min_length: usize,
    /// Randomly sample from the top K candidates (set 0 to sample from the full output distribution).
    pub sampling_topk: usize,
    /// Keep the most probable tokens whose cumulative probability exceeds this value.
    pub sampling_topp: f32,
    /// High temperature increase randomness.
    pub sampling_temperature: f32,
    /// Number of hypotheses to include in the result.
    pub num_hypotheses: usize,
    /// Include scores in the result.
    pub return_scores: bool,
    /// Return alternatives at the first unconstrained decoding position. This is typically
    /// used with a prefix to provide alternatives at a specifc location.
    pub return_alternatives: bool,
    /// Minimum probability to expand an alternative.
    pub min_alternative_expansion_prob: f32,
    /// The static prompt will prefix all inputs for this model.
    pub static_prompt: Vec<U>,
    /// Cache the model state after the static prompt and reuse it for future runs using
    /// the same static prompt.
    pub cache_static_prompt: bool,
    /// Include the input tokens in the generation result.
    pub include_prompt_in_result: bool,
    /// The maximum batch size. If the number of inputs is greater than `max_batch_size`,
    /// the inputs are sorted by length and split by chunks of `max_batch_size` examples
    /// so that the number of padding positions is minimized.
    pub max_batch_size: usize,
    /// Whether `max_batch_size` is the number of `examples` or `tokens`.
    pub batch_type: BatchType,
}

impl Default for GenerationOptions<String, String> {
    fn default() -> Self {
        Self {
            beam_size: 1,
            patience: 1.,
            length_penalty: 1.,
            repetition_penalty: 1.,
            no_repeat_ngram_size: 0,
            disable_unk: false,
            suppress_sequences: vec![],
            return_end_token: false,
            max_length: 512,
            min_length: 0,
            sampling_topk: 1,
            sampling_topp: 1.,
            sampling_temperature: 1.,
            num_hypotheses: 1,
            return_scores: false,
            return_alternatives: false,
            min_alternative_expansion_prob: 0.,
            static_prompt: vec![],
            cache_static_prompt: true,
            include_prompt_in_result: true,
            max_batch_size: 0,
            batch_type: Default::default(),
        }
    }
}

impl<T: AsRef<str>, U: AsRef<str>> GenerationOptions<T, U> {
    #[inline]
    fn to_ffi(&self) -> ffi::GenerationOptions {
        ffi::GenerationOptions {
            beam_size: self.beam_size,
            patience: self.patience,
            length_penalty: self.length_penalty,
            repetition_penalty: self.repetition_penalty,
            no_repeat_ngram_size: self.no_repeat_ngram_size,
            disable_unk: self.disable_unk,
            suppress_sequences: vec_ffi_vecstr(self.suppress_sequences.as_ref()),
            return_end_token: self.return_end_token,
            max_length: self.max_length,
            min_length: self.min_length,
            sampling_topk: self.sampling_topk,
            sampling_topp: self.sampling_topp,
            sampling_temperature: self.sampling_temperature,
            num_hypotheses: self.num_hypotheses,
            return_scores: self.return_scores,
            return_alternatives: self.return_alternatives,
            min_alternative_expansion_prob: self.min_alternative_expansion_prob,
            static_prompt: self.static_prompt.iter().map(|v| v.as_ref()).collect(),
            cache_static_prompt: self.cache_static_prompt,
            include_prompt_in_result: self.include_prompt_in_result,
            max_batch_size: self.max_batch_size,
            batch_type: self.batch_type,
        }
    }
}

/// A generation result.
#[derive(Clone, Debug)]
pub struct GenerationResult {
    /// Generated sequences of tokens.
    pub sequences: Vec<Vec<String>>,
    /// Generated sequences of token IDs.
    pub sequences_ids: Vec<Vec<usize>>,
    /// Score of each sequence (empty if `return_scores` was disabled).
    pub scores: Vec<f32>,
}

impl From<ffi::GenerationResult> for GenerationResult {
    fn from(res: ffi::GenerationResult) -> Self {
        Self {
            sequences: res.sequences.into_iter().map(|c| c.v).collect(),
            sequences_ids: res.sequences_ids.into_iter().map(|c| c.v).collect(),
            scores: res.scores,
        }
    }
}

impl GenerationResult {
    /// Returns the number of sequences.
    #[inline]
    pub fn num_sequences(&self) -> usize {
        self.sequences.len()
    }

    /// Returns true if this result has scores.
    #[inline]
    pub fn has_scores(&self) -> bool {
        !self.scores.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::generator::GenerationOptions;

    #[test]
    fn default_generation_options() {
        let options = GenerationOptions::default();

        assert_eq!(options.beam_size, 1);
        assert_eq!(options.patience, 1.);
        assert_eq!(options.length_penalty, 1.);
        assert_eq!(options.repetition_penalty, 1.);
        assert_eq!(options.no_repeat_ngram_size, 0);
        assert!(!options.disable_unk);
        assert!(options.suppress_sequences.is_empty());
        assert!(!options.return_end_token);
        assert_eq!(options.max_length, 512);
        assert_eq!(options.min_length, 0);
        assert_eq!(options.sampling_topk, 1);
        assert_eq!(options.sampling_topp, 1.);
        assert_eq!(options.sampling_temperature, 1.);
        assert_eq!(options.num_hypotheses, 1);
        assert!(!options.return_scores);
        assert!(!options.return_alternatives);
        assert_eq!(options.min_alternative_expansion_prob, 0.);
        assert!(options.static_prompt.is_empty());
        assert!(options.cache_static_prompt);
        assert!(options.include_prompt_in_result);
        assert_eq!(options.max_batch_size, 0);
        assert_eq!(options.batch_type, Default::default());
    }
}
