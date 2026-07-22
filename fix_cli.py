import re

with open("crates/vector-cli/src/main.rs", "r") as f:
    code = f.read()

new_block = """    let content = fs::read_to_string(input_file)?;
    let mut doc = SvgParser::parse_str(&content)?;

    // 0. Detect and normalize background
    if vector_algo::BackgroundProcessor::normalize_background(&mut doc) {
        debug!("Background detected and normalized for {}", input_file.display());
    }

    // 1. Execute ColorCombiner"""

code = code.replace("""    let content = fs::read_to_string(input_file)?;
    let doc = SvgParser::parse_str(&content)?;

    // 1. Execute ColorCombiner""", new_block)

with open("crates/vector-cli/src/main.rs", "w") as f:
    f.write(code)
