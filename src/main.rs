use std::env;
use std::fs;
use std::process;
use std::collections::HashMap;

use crate::args::{ CLIOption, update_config };

/* DEFAULT VALUES */

#[derive(Clone, Copy)]
pub struct ScriptTag<'a> {
  head: &'a str,
  tail: &'a str
}

#[derive(Clone, Copy)]
pub struct OutputDir<'a> {
  name: &'a str,
  mark: &'a str
}

pub static SRC: &str = "src.txt"; /* source filename (incl. output basename) */
pub static TAG: ScriptTag = ScriptTag { head: "###", tail: "#" }; /* tag line opener and optional label closer */
pub static DIR: OutputDir = OutputDir { name: "scripts", mark: ">" }; /* output directory name and placeholder */

/* TRANSFORMATION */

/* data structures */

/* - configuration */

#[derive(PartialEq, Eq)]
pub enum ConfigMapVal {
  Bool,
  Ints(Vec<usize>),
  Strs(Vec<String>)
}

pub type ConfigMap = HashMap<String, ConfigMapVal>;

pub struct Config<'a> {
  src: String,
  tag: ScriptTag<'a>,
  dir: OutputDir<'a>,
  map: ConfigMap
}

/* - consolidation */

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
struct OutputFile {
  data: Vec<String>,
  code: String,
  path: OutputFilePath,
  init: OutputFileInit,
  i: usize
}

impl OutputFile {
  fn new(data: Vec<String>, code: String, i: usize, config: &Config) -> OutputFile {

    let Config { src, tag: _, dir, map } = config;

    /* set output path parts */

    /* get output path parts - break first data item on '/' */
    let mut parts_path = data.get(0).unwrap().split('/').collect::<Vec<&str>>();

    /* handle option - dest - update output directory name */
    let dirname = if !map.contains_key("dest") { dir.name } else {
      match config.map.get("dest").unwrap() {
        ConfigMapVal::Strs(val_strs) => val_strs[0].as_str(),
        _                            => dir.name
      }
    };

    /* handle output directory identified by directory placeholder */
    if dir.mark == parts_path[0] { parts_path[0] = dirname };
    /* get output filename parts - separate last output path part and break on '.' */
    let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split('.').collect::<Vec<&str>>();
    let p_f_len = parts_filename.len();

    /* set as dir either remaining output path parts recombined or directory name */
    let dir = if !parts_path.is_empty() { parts_path.join("/") } else { dirname.to_string() };
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

  let config_init = Config { src: String::from(SRC), tag: TAG, dir: DIR, map: HashMap::new() };
  let args_by_cli = env::args().skip(1).collect::<Vec<String>>();
  let cli_options = Vec::from([
    CLIOption::new("dest", "d", &["DIR"], "set the default output directory to DIR", &apply_cli_option_dest),
    CLIOption::new("list", "l", &[], "print for each script in the source file its number and tag line label and data, skipping the save and run stages", &apply_cli_option_list),
    CLIOption::new("only", "o", &["SUBSET"], "include only scripts the numbers of which appear in SUBSET, comma-separated and/or in dash-indicated ranges, e.g. -o 1,3-5", &apply_cli_option_only),
    CLIOption::new("push", "p", &["LINE", "FILE"], "append to the source file LINE, auto-prefixed with a tag, followed by the content of FILE", &apply_cli_option_push),
    CLIOption::new_help()
  ]);

  let config_base = update_config(config_init, &cli_options, &apply_args_remaining, args_by_cli);

  /* load script file content or exit early */
  let content_whole = fs::read_to_string(&config_base.src).unwrap_or_else(|_| panic!("read source file '{}'", config_base.src));
  /* get args section plus each script with tag line minus tag head, numbered */
  let content_parts = content_whole.split(config_base.tag.head).enumerate().collect::<Vec<(usize, &str)>>();

  /* update config to encompass args section */
  let args_in_src = content_parts[0].1.split(' ').map(|part| part.trim().to_string()).filter(|part| part != "").collect::<Vec<String>>();
  let config_full = update_config(config_base, &cli_options, &apply_args_remaining, args_in_src);

  /* process each script plus tag line part */
  content_parts[1..].iter()
    .map(|(i, script_plus_tag_line_part)| (i, script_plus_tag_line_part, &config_full))
    /* handle option - only - use subset */
    .filter(|(i, _, config)| !config.map.contains_key("only") || match config.map.get("only").unwrap() {
      ConfigMapVal::Ints(val_ints) => val_ints.contains(&(i)),
      _                            => false
    })
    /* parse each item to output variant */
    .map(|(i, script_plus_tag_line_part, config)| parse(script_plus_tag_line_part, config, *i))
    /* print or save and run each variant */
    .for_each(apply)
}

fn parse(script_plus_tag_line_part: &str, config: &Config, i: usize) -> Option<Output> {

  let Config { src: _, tag, dir: _, map } = config;

  let mut lines = script_plus_tag_line_part.lines();
  let tag_line_part = lines.nth(0).unwrap();

  /* get label and data from tag line */
  let tag_line_sections = match tag_line_part.find(tag.tail) {
    Some(i) => tag_line_part.split_at(i + 1),
    None    => ("", tag_line_part)
  };
  let tag_line_label = tag_line_sections.0.split(tag.tail).nth(0).unwrap(); /* untrimmed */
  let tag_line_data  = tag_line_sections.1.trim();

  /* handle option - list - print only */
  if map.contains_key("list") {
    let join = if !tag_line_label.is_empty() { [tag_line_label, ":"].concat() } else { "".to_string() };
    let text = format!("{}:{} {}", i, join, tag_line_data);
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
    let text = format!("No tag data found for script no. {}", i);
    return Some(Output::Text(text));
  }
  if data.get(0).unwrap() == "!" {
    let text = format!("Bypassing script no. {} (! applied)", i);
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
  if prog == "!" { return println!("Not running file no. {} (! applied)", i); }
  if prog == "?" { return println!("Not running file no. {} (no values)", i); }

  /* run script from file */
  process::Command::new(&prog).args(args).arg(path)
    .spawn().unwrap_or_else(|_| panic!("run file with '{}'", prog))
    .wait_with_output().unwrap_or_else(|_| panic!("await output from '{}'", prog));
}

/* argument applicators */

fn apply_cli_option_dest(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {
  ConfigMapVal::Strs(Vec::from(strs))
}

fn apply_cli_option_list(_0: &Config, _1: &[CLIOption], _2: Vec<String>) -> ConfigMapVal {
  ConfigMapVal::Bool
}

fn apply_cli_option_only(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {
  let val_ints: Vec<usize> = strs[0].trim().split(',')
    .flat_map(|val_str| {
      let vals: Vec<usize> = val_str.trim().split('-').map(|item| item.parse::<usize>().expect("parse subset for option 'only'")).collect();
      if vals.len() > 1 { (vals[0]..(vals[1] + 1)).collect::<Vec<usize>>() } else { vals }
    })
    .collect();
  ConfigMapVal::Ints(val_ints)
}

fn apply_cli_option_push(config: &Config, _0: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {

  let script_filename = &strs[1];
  let Config { src, tag, dir: _, map: _ } = config;

  let script = fs::read_to_string(script_filename).unwrap_or_else(|_| panic!("read script file '{}'", script_filename));
  let script_plus_tag_line = format!("\n{} {}\n\n{}", tag.head, strs[0], script);

  use std::io::Write;
  let mut file = fs::OpenOptions::new().append(true).open(src).unwrap();
  file.write_all(&script_plus_tag_line.into_bytes()).expect("append script to source file");

  process::exit(0);
}

fn apply_args_remaining(mut config: Config, args_remaining: Vec<String>) -> Config {
  /* set final source filename (incl. output basename) per positional arg */
  config.src = if !args_remaining.is_empty() { String::from(&args_remaining[0]) } else { config.src };
  config
}

/* ARGUMENT HANDLING */

mod args {

  use std::process;
  use super::{ Config, ConfigMapVal };

  /* data structures */

  type CLIOptionCall = dyn Fn(&Config, &[CLIOption], Vec<String>) -> ConfigMapVal;

  pub struct CLIOption {
    word: String,
    char: String,
    strs: Vec<String>,
    desc: String,
    call: Box<CLIOptionCall>
  }

  impl CLIOption {
    pub fn new(word: &str, char: &str, val_strs: &[&str], desc: &str, call: &'static CLIOptionCall) -> CLIOption {
      CLIOption {
        word: String::from(word),
        char: String::from(char),
        strs: if !val_strs.is_empty() { val_strs.iter().map(|&val_str|String::from(val_str)).collect::<Vec<String>>() } else { Vec::new() },
        desc: String::from(desc),
        call: Box::new(call)
      }
    }

    pub fn new_help() -> CLIOption {
      CLIOption::new("help", "h", &[], "show usage and a list of available flags then exit", &apply_cli_option_help)
    }
  }

  type CLIArgHandler = dyn Fn(Config, Vec<String>) -> Config;

  /* argument applicator */

  fn apply_cli_option_help(_0: &Config, cli_options: &[CLIOption], _2: Vec<String>) -> ConfigMapVal {

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

  /* primary functions */

  pub fn update_config(mut config: Config<'static>, cli_options: &Vec<CLIOption>, handle_remaining: &CLIArgHandler, args: Vec<String>) -> Config<'static> {

    let args_count: usize = args.len().try_into().unwrap();

    /* for each flag in args, queue CLI option call with any values and tally */
    let mut cli_options_queued = Vec::new();
    let mut cli_options_count = 0;
    if args_count > 0 {
      for cli_option in cli_options {
        for j in 0..args_count {
          if "--".to_owned() + &cli_option.word == args[j] || "-".to_owned() + &cli_option.char == args[j] {
            let strs_len = cli_option.strs.len();
            let strs = args[(j + 1)..(j + strs_len + 1)].to_vec();
            cli_options_queued.push((&cli_option.word, &cli_option.call, strs));
            cli_options_count = cli_options_count + 1 + strs_len;
          };
        };
      };
    };

    /* make any queued CLI option calls */
    if !cli_options_queued.is_empty() {
      for opt_queued in &cli_options_queued {
        let (word, call, strs) = &opt_queued;
        let value = call(&config, &cli_options, strs.to_vec());
        config.map.insert(word.to_string(), value);
      }
    }

    /* handle any remaining arguments */
    let args_remaining = args[(cli_options_count)..].to_vec();
    config = handle_remaining(config, args_remaining);

    config
  }
}

/* UNIT TESTS */

#[cfg(test)]
mod test {

  use::std::collections::HashMap;
  use super::{ ScriptTag, OutputDir, Config, ConfigMapVal, Output, OutputFilePath, OutputFileInit, OutputFile, parse };

  fn get_values_parse() -> (Config<'static>, usize, String, OutputFilePath, OutputFileInit) {

    let src_default_str = "src.txt";
    let src_basename_default_str = src_default_str.split(".").nth(0).unwrap();

    let tag_default = ScriptTag { head: "###", tail: "#" };

    let dir_default_str = "scripts";
    let dir_default = OutputDir { name: dir_default_str, mark: ">" };

    let map_default = HashMap::new();

    let config_default = Config {
      src: String::from(src_default_str),
      tag: tag_default,
      dir: dir_default,
      map: map_default
    };

    /* base test script values */
    let index = 1;
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

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, i, code, path, init) = get_values_parse();
    let script_plus_tag_line_part = " label # ext program --flag value\n\n//code";
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_dest_option_some_output_file() {

    let (mut config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dest");
    let basename = String::from(config_default.src.split(".").nth(0).unwrap());
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    config_default.map.insert("dest".to_string(), ConfigMapVal::Strs(Vec::from([String::from("dest")])));

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_list_option_some_output_text() {

    let (mut config_default, i, _, _, _) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    config_default.map.insert("list".to_string(), ConfigMapVal::Bool);

    let expected = Option::Some(Output::Text(String::from("1: ext program --flag value")));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_singlepart_output_basename_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " script.ext program --flag value\n\n//code";

    let data = Vec::from(["script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir.name);
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_multipart_output_basename_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " script.suffix1.suffix2.ext program --flag value\n\n//code";

    let data = Vec::from(["script.suffix1.suffix2.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir.name);
    let basename = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
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

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = parse(script_plus_tag_line_part, &config_default, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_output_dir_placeholder_some_output_file() {

    let (config_default, i, code, _, init) = get_values_parse();
    let script_plus_tag_line_part = " >/script.ext program --flag value\n\n//code";

    let data = Vec::from([">/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("scripts");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
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

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));

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
