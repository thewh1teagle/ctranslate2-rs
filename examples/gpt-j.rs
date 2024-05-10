// gpt-j.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Generate text using GPT-J models.
//!
//! In this example, we will use
//! the [GPT-J](https://huggingface.co/docs/transformers/model_doc/gptj) model
//! to generate text.
//!
//! The original Python version of the code can be found in the
//! [CTranslate2 documentation](https://opennmt.net/CTranslate2/guides/transformers.html#gpt-j).
//!
//! First, convert the model files with the following command:
//!
//! ```bash
//! pip install -U ctranslate2 huggingface_hub torch transformers
//!
//! ct2-transformers-converter --model EleutherAI/gpt-j-6B --revision float16 \
//!     --quantization float16 --output_dir gpt-j-6B --copy_files tokenizer.json
//! ```
//!
//! Note: The above command copies `tokenizer.json` because it is provided by the
//! [EleutherAI/gpt-j-6b](https://huggingface.co/EleutherAI/gpt-j-6b) repository.
//! If you prefer to use another repository that offers `source.spm` and `target.spm`,
//! you can copy it using the option `--copy_files source.spm target.spm`.
//!
//! Create a file named `prompt.txt`, write the prompt, and save the file.
//! Then, execute the sample code below with the following command:
//!
//! ```bash
//! cargo run --example gpt-j -- ./gpt-j-6B
//! ```
//!

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time;

use anyhow::Result;
use clap::Parser;

use ct2rs::{GenerationOptions, Generator};
use ct2rs::config::{Config, Device};

/// Generate text using GPT-J models.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the file contains prompts.
    #[arg(short, long, value_name = "FILE", default_value = "prompt.txt")]
    prompt: String,
    /// Use CUDA.
    #[arg(short, long)]
    cuda: bool,
    /// Path to the directory that contains model.bin.
    path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let cfg = if args.cuda {
        Config {
            device: Device::CUDA,
            device_indices: vec![0],
            ..Config::default()
        }
    } else {
        Config::default()
    };

    let g = Generator::new(&args.path, &cfg)?;
    let prompts =
        BufReader::new(File::open(args.prompt)?)
            .lines()
            .fold(Ok(String::new()), |acc, line| {
                acc.and_then(|mut acc| {
                    line.map(|l| {
                        acc.push_str(&l);
                        acc
                    })
                })
            })?;

    let now = time::Instant::now();
    let res = g.generate_batch(
        &vec![prompts],
        &GenerationOptions {
            max_length: 30,
            sampling_topk: 10,
            ..GenerationOptions::default()
        },
        None,
    )?;
    let elapsed = now.elapsed();

    for (r, _) in res {
        println!("{}", r.join("\n"));
    }
    println!("Time taken: {elapsed:?}");

    Ok(())
}
