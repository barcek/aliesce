use std::env;
use std::fs;
use std::process;
use std::collections::HashMap;

/* DEFAULT VALUES */

#[derive(Clone, Copy)]
pub struct ScriptTag<'a> {
  head: &'a str,
  tail: &'a str
}

pub static SRC: &str = "src.txt"; /* source filename (incl. output basename) */
pub static DIR: &str = "scripts"; /* output directory */
pub static TAG: ScriptTag = ScriptTag { head: "###", tail: "#" };

/* TRANSFORMATION */

/* data structures */

#[derive(Debug, PartialEq)]
struct OutputFilePath {
  dir: String,
  basename: String,
  ext: String
}

impl OutputFilePath {
  fn get(&self) -> String {
    format!("{}/{}.{}", &self.dir, &self.basename, &self.ext)
  }
}

#[derive(Debug, PartialEq)]
struct OutputFileInit {
  prog: String,
  args: Vec<String>
}

#[derive(Debug, PartialEq)]
enum Output {
  Text(String),
  File(OutputFile)
}

#[derive(Debug, PartialEq)]
struct  OutputFile {
  data: Vec<String>,
  code: String,
  path: OutputFilePath,
  init: OutputFileInit,
  i: usize
}

impl OutputFile {
  fn new(data: Vec<String>, code: String, i: usize, config: &Config) -> OutputFile {

    let Config { src, tag: _, dir, opt_vals: _ } = config;

    /* set output path parts */

    /* get output path parts - break first data item on '/' */
    let mut parts_path = data.get(0).unwrap().split('/').collect::<Vec<&str>>();
    /* get output filename parts - separate last output path part and break on '.' */
    let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split('.').collect::<Vec<&str>>();
    let p_f_len = parts_filename.len();

    /* set as dir either remaining output path parts recombined or default dir */
    let dir = if !parts_path.is_empty() { parts_path.join("/") } else { dir.to_string() };
    /* set as basename either all but last output filename part or src basename */
    let basename = if p_f_len > 1 { parts_filename[..(p_f_len - 1)].join(".") } else { src.split('.').nth(0).unwrap().to_string() };
    /* set as ext last output filename part */
    let ext = parts_filename.iter().last().unwrap().to_string();

    /* set output init parts */

    /* set as prog tag line second item else '?' indicating absence (cf. function exec below) */
    let prog = if data.len() != 1 { data.get(1).unwrap().to_owned() } else { "?".to_string() };
    /* set as args Vec containing tag line remaining items */
    let args = data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>();

    /* assemble return value */

    let path = OutputFilePath{ dir, basename, ext };
    let init = OutputFileInit{ prog, args };

    OutputFile { data, code, path, init, i }
  }
}

/* primary functions */

fn main() {

  let config = get_config();

  /* load script file content or exit early */
  fs::read_to_string(&config.src).unwrap_or_else(|_| panic!("read source file '{}'", config.src))
    /* get each script with tag line minus tag head, omitting content preceding first */
    .split(config.tag.head).skip(1)
    /* yield also index (i) for each item */
    .enumerate()
    /* use subset if only option selected */
    .filter(|(i, _)| !config.opt_vals.contains_key("only") || match config.opt_vals.get("only").unwrap() {
      CLIOptVal::Ints(val_ints) => val_ints.contains(&(i + 1)),
      _                         => false
    })
    /* parse each item to output variant */
    .map(|(i, script_plus_tag_line_part)| parse(script_plus_tag_line_part, &config, i))
    /* print or save and run each variant */
    .for_each(apply)
}

fn parse(script_plus_tag_line_part: &str, config: &Config, i: usize) -> Option<Output> {

  let Config { src: _, tag, dir: _, opt_vals } = config;

  let mut lines = script_plus_tag_line_part.lines();
  let tag_line_part = lines.nth(0).unwrap();

  /* get label and data from tag line */
  let tag_line_sections = match tag_line_part.find(tag.tail) {
    Some(i) => tag_line_part.split_at(i + 1),
    None    => ("", tag_line_part)
  };
  let tag_line_label = tag_line_sections.0.split(tag.tail).nth(0).unwrap(); /* untrimmed */
  let tag_line_data  = tag_line_sections.1.trim();

  /* handle option selected - list */
  if opt_vals.contains_key("list") { /* account for list option */
    let join = if !tag_line_label.is_empty() { [tag_line_label, ":"].concat() } else { "".to_string() };
    let text = format!("{}:{} {}", i + 1, join, tag_line_data);
    return Some(Output::Text(text));
  };

  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");

  /* get items from tag line data */
  let data = tag_line_data.split(' ')
    .map(|item| item.to_string())
    .filter(|item| item != "") /* remove whitespace */
    .collect::<Vec<String>>();

  /* handle data absent or bypass */
  if data.is_empty() {
    let text = format!("No tag data found for script no. {}", i + 1);
    return Some(Output::Text(text));
  }
  if data.get(0).unwrap() == "!" {
    let text = format!("Bypassing script no. {} (! applied)", i + 1);
    return Some(Output::Text(text));
  }

  Some(Output::File(OutputFile::new(data, code, i, &config)))
}

fn apply(output: Option<Output>) {
  match output {
    Some(Output::Text(s)) => { println!("{}", &s); },
    Some(Output::File(s)) => { save(&s); exec(&s); },
    None                  => { return }
  };
}

fn save(output: &OutputFile) {

  let OutputFile { data: _, code, path, init: _, i: _ } = output;
  let dir = &path.dir;
  let path = path.get();

  /* add directory if none */
  fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("create directory '{}'", &dir));
  /* write script to file */
  fs::write(&path, code).unwrap_or_else(|_| panic!("write script to '{}'", &path));
}

fn exec(output: &OutputFile) {

  let OutputFile { data: _, code: _, path, init, i } = output;
  let OutputFileInit { prog, args } = init;
  let path = path.get();

  /* handle run precluded */
  if prog == "!" { return println!("Not running file no. {} (! applied)", i + 1); }
  if prog == "?" { return println!("Not running file no. {} (no values)", i + 1); }

  /* run script from file */
  process::Command::new(&prog).args(args).arg(path)
    .spawn().unwrap_or_else(|_| panic!("run file with '{}'", prog))
    .wait_with_output().unwrap_or_else(|_| panic!("await output from '{}'", prog));
}

/* CONFIGURATION */

/* data structures */

struct Config<'a> {
  src: String,
  tag: ScriptTag<'a>,
  dir: &'a str,
  opt_vals: CLIOptValMap
}

#[derive(PartialEq, Eq)]
enum CLIOptVal {
  Bool,
  Ints(Vec<usize>)
}

type CLIOptValMap = HashMap<String, CLIOptVal>;

type CLIOptCall = dyn Fn(&Config, &[CLIOpt], Vec<String>) -> CLIOptVal;

struct CLIOpt {
  word: String,
  char: String,
  strs: Vec<String>,
  desc: String,
  call: Box<CLIOptCall>
}

impl CLIOpt {
  fn new(word: &str, char: &str, val_strs: &[&str], desc: &str, call: &'static CLIOptCall) -> CLIOpt {
    CLIOpt {
      word: String::from(word),
      char: String::from(char),
      strs: if !val_strs.is_empty() { val_strs.iter().map(|&val_str|String::from(val_str)).collect::<Vec<String>>() } else { Vec::new() },
      desc: String::from(desc),
      call: Box::new(call)
    }
  }
}

/* primary functions */

fn apply_cli_option_list(_0: &Config, _1: &[CLIOpt], _2: Vec<String>) -> CLIOptVal {
  CLIOptVal::Bool
}

fn apply_cli_option_only(_0: &Config, _1: &[CLIOpt], strs: Vec<String>) -> CLIOptVal {
  let val_ints: Vec<usize> = strs[0].trim().split(',')
    .flat_map(|val_str| {
      let vals: Vec<usize> = val_str.trim().split('-').map(|item| item.parse::<usize>().expect("parse subset for option 'only'")).collect();
      if vals.len() > 1 { (vals[0]..(vals[1] + 1)).collect::<Vec<usize>>() } else { vals }
    })
    .collect();
  CLIOptVal::Ints(val_ints)
}

fn apply_cli_option_push(config: &Config, _0: &[CLIOpt], strs: Vec<String>) -> CLIOptVal {

  let script_filename = &strs[1];
  let Config { src, tag, dir: _, opt_vals: _ } = config;

  let script = fs::read_to_string(script_filename).unwrap_or_else(|_| panic!("read script file '{}'", script_filename));
  let script_plus_tag_line = format!("\n{} {}\n\n{}", tag.head, strs[0], script);

  use std::io::Write;
  let mut file = fs::OpenOptions::new().append(true).open(src).unwrap();
  file.write_all(&script_plus_tag_line.into_bytes()).expect("append script to source file");

  process::exit(0);
}

fn apply_cli_option_help(_0: &Config, cli_options: &[CLIOpt], _2: Vec<String>) -> CLIOptVal {

  /* set value substrings and max length */
  let val_strs = cli_options.iter()
    .map(|cli_option| cli_option.strs.join(" "))
    .collect::<Vec<String>>();
  let val_strs_max = val_strs.iter()
    .fold(0, |acc, val_str| if val_str.len() > acc { val_str.len() } else { acc });

  /* generate usage line */
  let usage_part = cli_options.iter()
    .filter(|cli_option| cli_option.word != "help") /* avoid duplication */
    .enumerate() /* yield also index (i) */
    .map(|(i, cli_option)| format!("[--{}/-{}{}]", cli_option.word, cli_option.char, if val_strs.is_empty() { "".to_owned() } else { " ".to_owned() + &val_strs[i] }))
    .collect::<Vec<String>>()
    .join(" ");
  let usage_line = format!("Usage: aliesce [--help/-h / {} [src]]", usage_part);

  /* generate flags list */
  let flags_list = cli_options.iter()
    .enumerate() /* yield also index (i) */
    .map(|(i, cli_option)| format!(" -{}, --{}  {:w$}  {}", cli_option.char, cli_option.word, val_strs[i], cli_option.desc, w = val_strs_max))
    .collect::<Vec<String>>()
    .join("\n");

  println!("{}\n{}\n{}", usage_line, String::from("Flags:"), flags_list);
  process::exit(0);
}

fn get_config() -> Config<'static> {

  /* set CLI options */
  let cli_options = [
    CLIOpt::new("list", "l", &[], "print for each script in the source file its number and tag line label and data, skipping the save and run stages", &apply_cli_option_list),
    CLIOpt::new("only", "o", &["SUBSET"], "include only scripts the numbers of which appear in SUBSET, comma-separated and/or in dash-indicated ranges, e.g. -o 1,3-5", &apply_cli_option_only),
    CLIOpt::new("push", "p", &["LINE", "FILE"], "append to the source file LINE, auto-prefixed with a tag, followed by the content of FILE", &apply_cli_option_push),
    CLIOpt::new("help", "h", &[], "show usage and a list of available flags then exit", &apply_cli_option_help)
  ];

  /* get CLI arguments and set count */
  let args: Vec<String> = env::args().collect();
  let mut args_count: usize = args.len().try_into().unwrap();

  /* for each flag passed queue option call with any values and tally */
  let mut opts_queued = Vec::new();
  let mut opts_count = 0;
  if args_count > 1 {
    for cli_option in &cli_options {
      for j in 1..args_count {
        if "--".to_owned() + &cli_option.word == args[j] || "-".to_owned() + &cli_option.char == args[j] {
          let strs_len = cli_option.strs.len();
          let strs = args[(j + 1)..(j + strs_len + 1)].to_vec();
          opts_queued.push((&cli_option.word, &cli_option.call, strs));
          opts_count = opts_count + 1 + strs_len;
        };
      };
    };
  };
  args_count -= args_count;

  /* set final source filename (incl. output basename), get script tag and output directory and initialize option values */
  let mut config = Config {
    src: if args_count > 1 { String::from(args.last().unwrap()) } else { String::from(SRC) },
    tag: TAG,
    dir: DIR,
    opt_vals: HashMap::new()
  };

  /* make any queued option calls */
  if !opts_queued.is_empty() {
    for opt_queued in &opts_queued {
      let (word, call, strs) = &opt_queued;
      let value = call(&config, &cli_options, strs.to_vec());
      config.opt_vals.insert(word.to_string(), value);
    }
  }

  config
}

/* UNIT TESTS */

#[cfg(test)]
mod test {

  use::std::collections::HashMap;
  use super::{ ScriptTag, Config, CLIOptVal, Output, OutputFilePath, OutputFileInit, OutputFile, parse };

  fn get_values_parse() -> (Config<'static>, usize, String, OutputFilePath, OutputFileInit) {

    let src_default_str = "src.txt";
    let src_basename_default_str = src_default_str.split(".").nth(0).unwrap();
    let tag_default = ScriptTag { head: "###", tail: "#" };
    let dir_default_str = "scripts";
    let opt_vals_default = HashMap::new();

    let config_default = Config {
      src: String::from(src_default_str),
      tag: tag_default,
      dir: dir_default_str,
      opt_vals: opt_vals_default
    };

    /* base test script values */
    let index = 0;
    let ext   = String::from("ext");
    let prog  = String::from("program");
    let args  = Vec::from([String::from("--flag"), String::from("value")]);
    let code  = String::from("//code");

    let output_path = OutputFilePath {
      dir: String::from(dir_default_str),
      basename: String::from(src_basename_default_str),
      ext
    };
    let output_init = OutputFileInit { prog, args };

    (config_default, index, code, output_path, output_init)
  }

  #[test]
  fn parse_returns_for_tag_data_full_some_output() {

    let (config_default, i, code, path, init) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, i, code, path, init) = get_values_parse();
    let script_plus_tag_line_part = " label # ext program --flag value\n\n//code";
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_list_option_some_output_text() {

    let (mut config_default, i, _, _, _) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    config_default.opt_vals.insert("list".to_string(), CLIOptVal::Bool);

    let expected = Option::Some(Output::Text(String::from("1: ext program --flag value")));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_singlepart_output_basename_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " script.ext program --flag value\n\n//code";
    let data = Vec::from(["script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir);
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_multipart_output_basename_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " script.suffix1.suffix2.ext program --flag value\n\n//code";
    let data = Vec::from(["script.suffix1.suffix2.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir);
    let basename = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_output_dir_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " dir/script.ext program --flag value\n\n//code";
    let data = Vec::from(["dir/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dir");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_minus_cmd_some_output_file_indicating() {

    let (config_default, i, code, path, _) = get_values_parse();
    let script_plus_tag_line_part = " ext\n\n//code";
    let data = Vec::from(["ext".to_string()]);

    let prog = String::from("?");
    let args = Vec::from([]);
    let init = OutputFileInit { prog, args };

    let expected = Option::Some(Output::File(OutputFile { data, code, path: path, init: init, i }));

    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_with_bypass_some_output_text() {

    let (config_default, i, _, _, _) = get_values_parse();
    let script_plus_tag_line_part = " ! ext program --flag value\n\n//code";

    let expected = Option::Some(Output::Text(String::from("Bypassing script no. 1 (! applied)")));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_absent_some_output_text() {

    let (config_default, i, _, _, _) = get_values_parse();
    let script_plus_tag_line_part = "\n\n//code";

    let expected = Option::Some(Output::Text(String::from("No tag data found for script no. 1")));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }
}
