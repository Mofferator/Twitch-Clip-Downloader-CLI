use clap_markdown::help_markdown;
use twdl::cli::Cli;
use std::fs;

fn main() {
    let md = help_markdown::<Cli>();

    let readme = fs::read_to_string("README.md").expect("readme missing");
    let new_readme = regex::Regex::new(
        r"(?s)(<!-- CLI-DOCS-START -->).*?(<!-- CLI-DOCS-END -->)"
    )
    .unwrap()
    .replace(
        &readme,
        format!("$1\n\n{}\n\n$2", md).as_str()
    );

    fs::write("README.md", new_readme.as_ref()).unwrap();
}
