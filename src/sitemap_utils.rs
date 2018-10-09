extern crate mfnf_sitemap;
extern crate serde_yaml;
#[macro_use]
extern crate structopt;
extern crate mwparser_utils;

use std::collections::HashMap;
use std::fmt::Write;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

use mwparser_utils::filename_to_make;

/// Extract information in various formats from a sitemap.
#[derive(StructOpt, Debug)]
#[structopt(
    name = "sitemap_utils",
    about = "various tools for \
             extracting information from a mfnf sitemap"
)]
struct Opt {
    /// Input sitemap (yaml) file
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input_file: Option<PathBuf>,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Generate a list of dependencies for this sitemap
    #[structopt(name = "deps")]
    Deps {
        /// The target to build for.
        #[structopt(name = "target")]
        target: String,

        /// The subtarget (configuration) to consider.
        #[structopt(name = "subtarget")]
        subtarget: String,
    },
    /// Get markers for an article.
    /// Also prepend subtargets with target, like: print -> latex.print.
    #[structopt(name = "markers")]
    Markers {
        /// Name of the article to get markers for.
        #[structopt(name = "article")]
        article: String,

        /// The target to prepend
        #[structopt(name = "target")]
        target: String,
    },
}

fn main() {
    let opt = Opt::from_args();

    let mut target_extension_map = HashMap::new();
    target_extension_map.insert("latex".to_string(), "tex");
    target_extension_map.insert("markdown".to_string(), "md");
    target_extension_map.insert("html".to_string(), "html");
    target_extension_map.insert("pdf".to_string(), "pdf");
    target_extension_map.insert("stats".to_string(), "stats.yml");

    let mut input = String::new();
    match opt.input_file {
        Some(path) => {
            BufReader::new(File::open(&path).expect("Could not open input file!"))
                .read_to_string(&mut input)
                .expect("Could not read input file!");
        }
        None => {
            BufReader::new(io::stdin())
                .read_to_string(&mut input)
                .expect("Could not read input file!");
        }
    };

    let sitemap: mfnf_sitemap::Book = serde_yaml::from_str(&input).expect("Error parsing sitemap:");

    match opt.cmd {
        Command::Deps {
            ref subtarget,
            ref target,
        } => {
            let subtarget = subtarget.trim().to_lowercase();

            match target.as_str() {
                "pdf" => {
                    println!(
                        "$(BOOK_REVISION).pdf: $(BASE)/book_exports/\
                         $(BOOK)/$(BOOK_REVISION)/latex/{}/$(BOOK_REVISION).tex",
                        &subtarget
                    );
                    return;
                }
                _ => (),
            };

            let article_extension = match target.as_str() {
                "latex" => "tex",
                "html" => "html",
                "pdf" => "pdf",
                "stats" => "stats.yml",
                _ => panic!("undefined target: {}", &target),
            };

            // collects statements for including per-article dep files
            let mut include_string = String::new();
            // collects dependencies for book anchors file.
            let mut anchors_string = String::new();

            print!("$(BOOK_REVISION).{}: ", &article_extension);
            write!(&mut anchors_string, "$(BOOK_REVISION).anchors: ");

            for part in &sitemap.parts {
                for chapter in &part.chapters {
                    let exclude_subtarget = chapter
                        .markers
                        .exclude
                        .subtargets
                        .iter()
                        .find(|t| t.name == subtarget);

                    // is the subtarget only partially excluded?
                    let included = if let Some(subtarget) = exclude_subtarget {
                        !subtarget.parameters.is_empty()
                    // target not excluded
                    } else {
                        true
                    };

                    if included {
                        let chapter_path = filename_to_make(&chapter.path);

                        print!(
                            "{0}/{1}.media-dep {0}/{1}.section-dep ",
                            &chapter_path, &chapter.revision
                        );

                        write!(
                            &mut anchors_string,
                            "{0}/{1}.anchors ",
                            &chapter_path, &chapter.revision
                        );

                        write!(
                            &mut include_string,
                            "include {0}/{1}.section-dep\n-include {0}/{1}.media-dep\n",
                            &chapter_path, &chapter.revision
                        );

                        match target.as_str() {
                            "latex" => print!("{0}/{1}.tex ", &chapter_path, &chapter.revision),
                            "stats" => print!(
                                "{0}/{1}.stats.yml {0}/{1}.lints.yml ",
                                &chapter_path, &chapter.revision
                            ),
                            "html" => print!("{0}/{1}.html ", &chapter_path, &chapter.revision),
                            _ => panic!("undefined target: {}", &target),
                        }
                    }
                }
            }
            println!();
            println!("{}", &anchors_string);
            println!("{}", &include_string);
        }
        Command::Markers {
            ref article,
            ref target,
        } => {
            let article = article.trim().to_lowercase();
            for part in &sitemap.parts {
                for chapter in &part.chapters {
                    if chapter.path.trim().to_lowercase() == article {
                        let mut markers = chapter.markers.clone();
                        let update_name = |name: &mut String| {
                            name.insert(0, '.');
                            name.insert_str(0, target);
                        };
                        let new_include = markers
                            .include
                            .subtargets
                            .drain(..)
                            .map(|mut subtarget| {
                                update_name(&mut subtarget.name);
                                subtarget
                            }).collect();
                        let new_exclude = markers
                            .exclude
                            .subtargets
                            .drain(..)
                            .map(|mut subtarget| {
                                update_name(&mut subtarget.name);
                                subtarget
                            }).collect();

                        markers.include.subtargets = new_include;
                        markers.exclude.subtargets = new_exclude;

                        println!(
                            "{}",
                            &serde_yaml::to_string(&markers).expect("could not serialize markers:")
                        );
                        return;
                    }
                }
            }
            eprintln!("chapter not found: {}", article);
            process::exit(1);
        }
    }
}
