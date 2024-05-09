// marian-mt.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, stdout, Write};
use std::time;

use anyhow::Result;
use clap::Parser;

use ct2rs::config::{Config, Device};
use ct2rs::sentencepiece::Tokenizer;
use ct2rs::Translator;

/// Translate a file using Marian-MT model.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the output file. If not specified, output to stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,
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
    let t = Translator::with_tokenizer(&args.path, Tokenizer::new(&args.path)?, &cfg)?;

    let sources = BufReader::new(File::open(args.prompt)?)
        .lines()
        .collect::<std::result::Result<Vec<String>, io::Error>>()?;

    let now = time::Instant::now();
    let res = t.translate_batch(&sources, &Default::default(), None)?;
    let elapsed = now.elapsed();

    let mut out: BufWriter<Box<dyn Write>> = BufWriter::new(match args.output {
        None => Box::new(stdout()),
        Some(p) => Box::new(File::create(p)?),
    });
    for (r, _) in res {
        writeln!(out, "{r}")?;
    }
    writeln!(out, "Time taken: {:?}", elapsed)?;

    Ok(())
}
