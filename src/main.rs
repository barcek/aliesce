/*
  SOURCE PROCESSING
  - imports
  - DEFAULT VALUES, incl. data structures
  - data structures, remaining
    - configuration
    - consolidation
  - utility functions, incl. DOC LINES
  - primary functions, incl. MAIN (w/ CLI OPTIONS)
  - argument applicators

  ARGUMENT HANDLING / mod args
  - imports
  - data structures
  - argument applicator ('help')
  - utility functions
  - primary functions

  UNIT TESTING / mod test
  - imports
  - test cases
*/

/* SOURCE PROCESSING */

/* - imports */

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time::Duration;
use std::env;
use std::fs;
use std::process;
use std::collections::HashMap;

use crate::args::{ CLIOption, config_update };

/* - DEFAULT VALUES, incl. data structures */

static SRC_PATH: &str = "src.txt";               /* source file path (incl. output basename) */
static DEST_DIR: &str = "scripts";               /* output directory name */
static TAG_LINE: ScriptTagLine = ScriptTagLine { /* tag line opener and label closer, signals, placeholders and command parts */
  tag_head:      "###",
  tag_tail:      "#",
  sig_stop:      "!",
  plc_path_dir:  ">",
  plc_path_all:  "><",
  cmd_prog:      "bash",
  cmd_flag:      "-c"
};

#[derive(Clone, Copy)]
struct ScriptTagLine<'a> {
  tag_head:     &'a str,
  tag_tail:     &'a str,
  sig_stop:     &'a str,
  plc_path_dir: &'a str,
  plc_path_all: &'a str,
  cmd_prog:     &'a str,
  cmd_flag:     &'a str
}

/* - data structures, remaining */

/*   - configuration */

#[derive(PartialEq, Eq)]
pub enum ConfigMapVal {
  Bool,
  Ints(Vec<usize>),
  Strs(Vec<String>)
}

pub type ConfigMap = HashMap<String, ConfigMapVal>;

pub struct Config<'a> {
  src: String,
  tag: ScriptTagLine<'a>,
  dir: &'a str,
  map: ConfigMap
}

/*   - consolidation */

struct Inputs<'a> {
  i: usize,
  srcstr: &'a str,
  config: &'a Config<'a>
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

    let Config { src, tag, dir, map } = config;

    /* set output path parts */

    /* get output path parts - break first data item on '/' */
    let mut parts_path = data.get(0).unwrap().split('/').collect::<Vec<&str>>();

    /* handle option - dest - update output directory name */
    let dirname = if !map.contains_key("dest") { dir } else {
      match config.map.get("dest").unwrap() {
        ConfigMapVal::Strs(val_strs) => val_strs[0].as_str(),
        _                            => dir
      }
    };

    /* handle output directory identified by directory placeholder */
    if tag.plc_path_dir == parts_path[0] { parts_path[0] = dirname };
    /* get output filename parts - separate last output path part and break on '.' */
    let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split('.').collect::<Vec<&str>>();
    let p_f_len = parts_filename.len();

    /* set as dir either remaining output path parts recombined or directory name */
    let dir = if !parts_path.is_empty() { parts_path.join("/") } else { dirname.to_string() };
    /* set as basename either all but last output filename part or src basename */
    let basename = if p_f_len > 1 { parts_filename[..(p_f_len - 1)].join(".") } else { src.split('.').nth(0).unwrap().to_string() };
    /* set as ext last output filename part */
    let ext = parts_filename.iter().last().unwrap().to_string();

    let path = OutputFilePath{ dir, basename, ext };

    /* set output init parts */

    /* handle file run precluded */
    if data.len() == 1 {
      let init = OutputFileInit::Text(format!("Not running file no. {} (no values)", i));
      return OutputFile { data, code, path, init, i };
    }
    if data.get(1).unwrap() == tag.sig_stop {
      let init = OutputFileInit::Text(format!("Not running file no. {} ({} applied)", i, tag.sig_stop));
      return OutputFile { data, code, path, init, i };
    }

    /* note presence of output path placeholder, as indicator of composite command */
    let has_placeholder = data.iter().skip(1).any(|item| tag.plc_path_all == item);

    /* set as prog either tag line second item or default */
    let prog = if has_placeholder { String::from(tag.cmd_prog) } else { data.get(1).unwrap().to_owned() };
    /* set as args either Vec containing remaining items plus combined path or default flag plus remaining items joined */
    let mut args = Vec::from([]);
    if has_placeholder {
      args.push(tag.cmd_flag.to_owned());
      args.push(data.iter().skip(1).map(|item| if tag.plc_path_all == item { path.get() } else { item.to_owned() }).collect::<Vec<String>>().join(" "));
    }
    else {
      args.append(&mut data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>());
      args.push(path.get());
    };

    let init = OutputFileInit::Code(OutputFileInitCode { prog, args });

    OutputFile { data, code, path, init, i }
  }
}

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
enum OutputFileInit {
  Text(String),
  Code(OutputFileInitCode)
}

#[derive(Debug, PartialEq)]
struct OutputFileInitCode {
  prog: String,
  args: Vec<String>
}

/* - utility functions, incl. DOC LINES */

fn doc_lines_get() -> [String; 5] {

  let form = format!("The default source file path is '{}'. Each script in the source file requires a preceding tag line. A tag line begins with the tag head ('{}') and has an optional label with the tag tail ('{}'). The format is shown below.", SRC_PATH, TAG_LINE.tag_head, TAG_LINE.tag_tail);
  let line = format!("{} <any label {}> <OUTPUT EXTENSION or FULL OUTPUT PATH: [[dirname(s)/]basename.]extension> <COMMAND incl. any arguments>", TAG_LINE.tag_head, TAG_LINE.tag_tail);

  let data_items = format!("By default the script is saved with the OUTPUT EXTENSION or to the FULL OUTPUT PATH then the COMMAND is run with the path generated. The '{}' placeholder can be used in the COMMAND to denote path position and pass the whole to '{} {}'.", TAG_LINE.plc_path_all, TAG_LINE.cmd_prog, TAG_LINE.cmd_flag);
  let data_chars = format!("The '{}' signal can be included before the OUTPUT EXTENSION or FULL OUTPUT PATH to avoid both the save and run stages, or before the COMMAND to save but avoid the run stage. The '{}' placeholder can be used in the FULL OUTPUT PATH to represent the default or overridden output directory name.", TAG_LINE.sig_stop, TAG_LINE.plc_path_dir);

  let read = format!("One or more paths can be piped to 'aliesce' to append the content at each to the source file as a script, auto-preceded by a tag line with '{}', then exit.", TAG_LINE.sig_stop);

  [form, line, data_items, data_chars, read]
}

fn script_push(config: &Config, strs: Vec<String>) {

  let script_filename = &strs[1];
  let Config { src, tag, dir: _, map: _ } = config;

  /* handle read */

  let script = fs::read_to_string(script_filename)
    .unwrap_or_else(|err| error_handle((&format!("Not parsing script file '{}'", script_filename), Some("read"), Some(err))));
  let tag_line = format!("{} {}", tag.tag_head, strs[0]);
  let script_plus_tag_line = format!("\n{}\n\n{}", tag_line, script);

  /* handle write */

  use io::Write;
  let sum_base = format!("tag line '{}' and content of script file '{}' to source file '{}'", tag_line, script_filename, src);
  let sum_failure = format!("Not appending {}", sum_base);
  let sum_success = format!("Appended {}", sum_base);

  let mut file = fs::OpenOptions::new().append(true).open(src)
    .unwrap_or_else(|err| error_handle((&sum_failure, Some("open"), Some(err))));
  file.write_all(&script_plus_tag_line.into_bytes())
    .unwrap_or_else(|err| error_handle((&sum_failure, Some("write"), Some(err))));
  println!("{}", sum_success);
}

fn error_handle(strs: (&String, Option<&str>, Option<io::Error>)) -> ! {
  match strs {
    (sum, Some(act), Some(err)) => eprintln!("{} ({} error: '{}')", sum, act, err),
    (sum, None, None)           => eprintln!("{}", sum),
    _                           => eprintln!("Failed (unknown error)")
  }
  process::exit(1);
}

/* - primary functions, incl. MAIN (w/ CLI OPTIONS) */

fn main() {

  /* set config per defaults, CLI options and args on CLI */
  let cli_options = cli_options_get();
  let args_on_cli = env::args().skip(1).collect::<Vec<String>>();
  let config_init = Config { src: String::from(SRC_PATH), tag: TAG_LINE, dir: DEST_DIR, map: HashMap::new() };
  let config_base = config_update(config_init, &cli_options, &args_remaining_cli_apply, args_on_cli);

  /* handle any pushes to script file for paths via stdin */
  let paths_stdin = stdin_read();
  if !paths_stdin.is_empty() {
    for path in paths_stdin {
      script_push(&config_base, Vec::from([TAG_LINE.sig_stop.to_string(), path]));
    }
    process::exit(0);
  };

  /* load script file content or exit early */
  let content_whole = fs::read_to_string(&config_base.src)
    .unwrap_or_else(|err| error_handle((&format!("Not parsing source file '{}'", config_base.src), Some("read"), Some(err))));

  /* get args section plus each source string (script with tag line minus tag head) numbered, excl. init option content */
  let [form, line, _, _, _] = &doc_lines_get();
  let content_added = content_whole.replace(form, "").replace(line, "");
  let mut content_parts = content_added.split(config_base.tag.tag_head).enumerate().collect::<Vec<(usize, &str)>>();

  /* remove any shebang line in args section */
  if &content_parts[0].1.len() >= &2 && "#!" == &content_parts[0].1[..2] {
    let remainder = content_parts[0].1.splitn(2, '\n').last().unwrap();
    content_parts[0] = (content_parts[0].0, remainder);
  }

  /* update config to encompass args section */
  let args_in_src = content_parts[0].1.split_whitespace().map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect::<Vec<String>>();
  let config_full = config_update(config_base, &cli_options, &args_remaining_src_apply, args_in_src);

  content_parts[1..].iter()
    /* process each part to input instance */
    .map(|(i, srcstr)| Inputs { i: *i, srcstr, config: &config_full })
    /* handle option - only - allow subset */
    .filter(inputs_match)
    /* parse each input to output instance */
    .map(inputs_parse)
    /* print output text or poss. use file */
    .for_each(output_apply)
}

fn stdin_read() -> Vec<String> {

  use io::Read;
  let (tx, rx) = mpsc::channel();

  thread::spawn(move || {
    let mut stdin = io::stdin();
    let mut bfr = String::new();
    stdin.read_to_string(&mut bfr).unwrap();
    tx.send(bfr).unwrap();
  });
  thread::sleep(Duration::from_millis(25));

  match rx.try_recv() {
    Ok(recvd) => recvd.split_whitespace().map(|str| str.to_string()).filter(|str| !str.is_empty()).collect::<Vec<String>>(),
    Err(_)    => Vec::new()
  }
}

fn cli_options_get() -> Vec<CLIOption> {
  Vec::from([
    CLIOption::new("dest", "d", &["DIR"], &*format!("set the default output directory name (currently '{}') to DIR", DEST_DIR), &cli_option_dest_apply),
    CLIOption::new("list", "l", &[], "print for each script in the source file its number and tag line content, skipping the save and run stages", &cli_option_list_apply),
    CLIOption::new("only", "o", &["SUBSET"], "include only scripts the numbers of which appear in SUBSET, comma-separated and/or in dash-indicated ranges, e.g. -o 1,3-5", &cli_option_only_apply),
    CLIOption::new("push", "p", &["LINE", "PATH"], "append to the source file LINE, auto-prefixed with a tag, followed by the content at PATH then exit", &cli_option_push_apply),
    CLIOption::new("init", "i", &[], &*format!("create a template source file at the default source file path (currently '{}') then exit", SRC_PATH), &cli_option_init_apply),
    CLIOption::new_help()
  ])
}

fn inputs_match(inputs: &Inputs) -> bool {
  !inputs.config.map.contains_key("only") || match inputs.config.map.get("only").unwrap() {
    ConfigMapVal::Ints(val_ints) => val_ints.contains(&(inputs.i)),
    _                            => false
  }
}

fn inputs_parse(inputs: Inputs) -> Option<Output> {

  let Inputs { i, srcstr, config } = inputs;
  let Config { src: _, tag, dir: _, map } = config;

  let mut lines = srcstr.lines();
  let tag_line_part = lines.nth(0).unwrap();

  /* get label and data from tag line */
  let tag_line_sections = match tag_line_part.find(tag.tag_tail) {
    Some(i) => tag_line_part.split_at(i + 1),
    None    => ("", tag_line_part)
  };
  let tag_line_label = tag_line_sections.0.split(tag.tag_tail).nth(0).unwrap(); /* untrimmed */
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
    .filter(|item| !item.is_empty()) /* remove whitespace */
    .collect::<Vec<String>>();

  /* handle data absent or bypass */
  if data.is_empty() {
    let text = format!("No tag data found for script no. {}", i);
    return Some(Output::Text(text));
  }
  if data.get(0).unwrap() == tag.sig_stop {
    let text = format!("Bypassing script no. {} ({} applied)", i, tag.sig_stop);
    return Some(Output::Text(text));
  }

  Some(Output::File(OutputFile::new(data, code, i, config)))
}

fn output_apply(output: Option<Output>) {
  match output {
    Some(Output::Text(s)) => { println!("{}", &s); },
    Some(Output::File(s)) => { output_save(&s); output_exec(&s); },
    None                  => {}
  };
}

fn output_save(output: &OutputFile) {

  let OutputFile { data: _, code, path, init: _, i: _ } = output;
  let dir = &path.dir;
  let path = path.get();

  /* add directory if none */
  fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("create directory '{}'", &dir));
  /* write script to file */
  fs::write(&path, code).unwrap_or_else(|_| panic!("write script to '{}'", &path));
}

fn output_exec(output: &OutputFile) {

  let OutputFile { data: _, code: _, path: _, init, i: _ } = output;

  match init {

    /* print reason file run precluded */
    OutputFileInit::Text(s) => println!("{}", s),

    /* run script from file */
    OutputFileInit::Code(c) => {
      let OutputFileInitCode { prog, args } = c;
      process::Command::new(&prog).args(args)
        .spawn().unwrap_or_else(|_| panic!("run file with '{}'", prog))
        .wait_with_output().unwrap_or_else(|_| panic!("await output from '{}'", prog));
    }
  }
}

/* - argument applicators */

fn cli_option_dest_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {
  ConfigMapVal::Strs(strs)
}

fn cli_option_list_apply(_0: &Config, _1: &[CLIOption], _2: Vec<String>) -> ConfigMapVal {
  ConfigMapVal::Bool
}

fn cli_option_only_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {
  let val_ints: Vec<usize> = strs[0].trim().split(',')
    .flat_map(|val_str| {
      let vals: Vec<usize> = val_str.trim().split('-').map(|item| item.parse::<usize>().expect("parse subset for option 'only'")).collect();
      if vals.len() > 1 { (vals[0]..(vals[1] + 1)).collect::<Vec<usize>>() } else { vals }
    })
    .collect();
  ConfigMapVal::Ints(val_ints)
}

fn cli_option_push_apply(config: &Config, _0: &[CLIOption], strs: Vec<String>) -> ConfigMapVal {
  script_push(config, strs);
  process::exit(0);
}

fn cli_option_init_apply(config: &Config, _0: &[CLIOption], _1: Vec<String>) -> ConfigMapVal {

  let [form, line, data_items, data_chars, read] = doc_lines_get();
  let src = &config.src;

  let content = format!("\
    <any arguments to aliesce (run 'aliesce --help' for options)>\n\n\
    Notes on source file format:\n\n\
    {}\n\n{}\n\n{}\n\n\
    Appending scripts via stdin:\n\n\
    {}\n\n\
    Tag line and script section:\n\n\
    {}\n\n<script>\n\
    ", form, data_items, data_chars, read, line
  );

  /* handle write */

  let sum_failure = format!("Not creating template source file at '{}'", src);

  /* exit early if source file exists */
  if fs::metadata(src).is_ok() { error_handle((&format!("{} (path exists)", sum_failure), None, None)) };

  fs::write(src, content).unwrap_or_else(|err| error_handle((&sum_failure, Some("write"), Some(err))));

  println!("Created template source file at '{}'", src);
  process::exit(0);
}

fn args_remaining_cli_apply(mut config: Config, args_remaining: Vec<String>) -> Config {
  /* set final source filename (incl. output basename) per positional arg */
  config.src = if !args_remaining.is_empty() { String::from(&args_remaining[0]) } else { config.src };
  config
}

fn args_remaining_src_apply(config: Config, _: Vec<String>) -> Config {
  config
}

/* ARGUMENT HANDLING */

mod args {

  /* - imports */

  use std::process;
  use super::{ Config, ConfigMapVal, doc_lines_get };

  /* - data structures */

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
      CLIOption::new("help", "h", &[], "show usage, flags available and notes then exit", &cli_option_help_apply)
    }
  }

  type CLIArgHandler = dyn Fn(Config, Vec<String>) -> Config;

  /* - argument applicator ('help') */

  fn cli_option_help_apply(_: &Config, cli_options: &[CLIOption], _0: Vec<String>) -> ConfigMapVal {

    /* set value substrings and max length */
    let val_strs = cli_options.iter()
      .map(|cli_option| cli_option.strs.join(" "))
      .collect::<Vec<String>>();
    let val_strs_max = val_strs.iter()
      .fold(0, |acc, val_str| if val_str.len() > acc { val_str.len() } else { acc });

    /* generate usage text */
    let usage_opts_part = cli_options.iter()
      .filter(|cli_option| cli_option.word != "help") /* avoid duplication */
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| format!("[--{}/-{}{}]", cli_option.word, cli_option.char, if val_strs.is_empty() { "".to_owned() } else { " ".to_owned() + &val_strs[i] }))
      .collect::<Vec<String>>()
      .join(" ");
    let usage_opts_full = line_break_and_indent(&format!("[--help/-h / {} [source file path]]", usage_opts_part), 15, 80, false);
    let usage_text = format!("Usage: aliesce {}", usage_opts_full);

    /* generate flags text */
    let flags_list = cli_options.iter()
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| {
        let desc = line_break_and_indent(&cli_option.desc, val_strs_max + 15, 80, false);
        format!(" -{}, --{}  {:w$}  {}", cli_option.char, cli_option.word, val_strs[i], desc, w = val_strs_max)
      })
      .collect::<Vec<String>>()
      .join("\n");
    let flags_text = format!("Flags:\n{}", flags_list);

    /* generate notes text */
    let notes_body = doc_lines_get().map(|line| line_break_and_indent(&line, 1, 80, true)).join("\n\n");
    let notes_text = format!("Notes:\n{}", notes_body);

    println!("{}\n{}\n{}", usage_text, flags_text, notes_text);
    process::exit(0);
  }

  /* - utility functions */

  fn line_break_and_indent(line: &str, indent: usize, length: usize, indent_first: bool ) -> String {

    let whitespace_part = String::from(" ").repeat(indent);
    let whitespace_full = format!("\n{}", whitespace_part);
    let text_width = length - indent;

    let body = line.split(' ').collect::<Vec<&str>>().iter()
      .fold(Vec::new(), |mut acc: Vec<String>, word| {
        if acc.is_empty() { return Vec::from([word.to_string()]) };
        /* accrue text part of each line by word, not exceeding text width */
        let index_last = acc.len() - 1;
        match acc[index_last].chars().count() + word.chars().count() >= text_width {
          /* begin new text part with word */
          true => acc.push(String::from(*word)),
          /* add word to current text part */
          _    => acc[index_last].push_str(&format!(" {}", *word))
        };
        acc
      })
      .join(whitespace_full.as_str());

    if indent_first { format!("{}{}", whitespace_part, body) } else { body }
  }

  /* - primary functions */

  pub fn config_update(mut config: Config<'static>, cli_options: &[CLIOption], handle_remaining: &CLIArgHandler, args: Vec<String>) -> Config<'static> {

    let args_count: usize = args.len();

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

    /* handle any remaining arguments */
    let args_remaining = args[(cli_options_count)..].to_vec();
    config = handle_remaining(config, args_remaining);

    /* make any queued CLI option calls */
    if !cli_options_queued.is_empty() {
      for opt_queued in &cli_options_queued {
        let (word, call, strs) = &opt_queued;
        let value = call(&config, cli_options, strs.to_vec());
        config.map.insert(word.to_string(), value);
      }
    }

    config
  }
}

/* UNIT TESTING */

#[cfg(test)]
mod test {

  /* - imports */

  use::std::collections::HashMap;
  use super::{
    ScriptTagLine,
    Config, ConfigMapVal,
    Inputs,
    Output, OutputFile, OutputFilePath, OutputFileInit, OutputFileInitCode,
    inputs_parse
  };

  /* - test cases */

  /*   - function: inputs_parse */

  fn get_values_for_parse_inputs() -> (Config<'static>, usize, String, OutputFilePath, OutputFileInit) {

    let src_default_str = "src.txt";
    let src_basename_default_str = src_default_str.split(".").nth(0).unwrap();

    let tag_default = ScriptTagLine { tag_head: "###", tag_tail: "#", sig_stop: "!", plc_path_dir: ">", plc_path_all: "><", cmd_prog: "bash", cmd_flag: "-c" };
    let dir_default = "scripts";

    let map_default = HashMap::new();

    let config_default = Config {
      src: String::from(src_default_str),
      tag: tag_default,
      dir: dir_default,
      map: map_default
    };

    /* base test script values */

    let ext = String::from("ext");

    let output_path = OutputFilePath {
      dir: String::from(dir_default),
      basename: String::from(src_basename_default_str),
      ext
    };

    let index = 1;
    let prog  = String::from("program");
    let args  = Vec::from([String::from("--flag"), String::from("value"), output_path.get()]);
    let code  = String::from("//code");

    let output_init = OutputFileInit::Code(OutputFileInitCode { prog, args });

    (config_default, index, code, output_path, output_init)
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_some_output() {

    let (config_default, i, code, path, init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, i, code, path, init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " label # ext program --flag value\n\n//code";
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_dest_option_some_output_file() {

    let (mut config_default, i, code, _, mut init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dest");
    let basename = String::from(config_default.src.split(".").nth(0).unwrap());
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };
    config_default.map.insert("dest".to_string(), ConfigMapVal::Strs(Vec::from([String::from("dest")])));

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_list_option_some_output_text() {

    let (mut config_default, i, _, _, _) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    config_default.map.insert("list".to_string(), ConfigMapVal::Bool);

    let expected = Option::Some(Output::Text(String::from("1: ext program --flag value")));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_incl_singlepart_output_basename_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " script.ext program --flag value\n\n//code";

    let data = Vec::from(["script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir);
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_incl_multipart_output_basename_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " script.suffix1.suffix2.ext program --flag value\n\n//code";

    let data = Vec::from(["script.suffix1.suffix2.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.dir);
    let basename = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_incl_output_dir_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " dir/script.ext program --flag value\n\n//code";

    let data = Vec::from(["dir/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dir");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_incl_output_path_dir_placeholder_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " >/script.ext program --flag value\n\n//code";

    let data = Vec::from([">/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("scripts");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, basename, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_incl_output_path_all_placeholder_some_output() {

    let (config_default, i, code, path, _) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ext program_1 --flag value >< | program_2\n\n//code";
    let data = Vec::from(["ext".to_string(), "program_1".to_string(), "--flag".to_string(), "value".to_string(), "><".to_string(), "|".to_string(), "program_2".to_string()]);

    let prog = String::from(config_default.tag.cmd_prog);
    let args = Vec::from([String::from(config_default.tag.cmd_flag), String::from("program_1 --flag value scripts/src.ext | program_2")]);
    let init = OutputFileInit::Code(OutputFileInitCode { prog, args });

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_minus_cmd_some_output_file_indicating() {

    let (config_default, i, code, path, _) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ext\n\n//code";

    let data = Vec::from(["ext".to_string()]);
    let init = OutputFileInit::Text(String::from("Not running file no. 1 (no values)"));

    let expected = Option::Some(Output::File(OutputFile { data, code, path, init, i }));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_full_with_bypass_some_output_text() {

    let (config_default, i, _, _, _) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = " ! ext program --flag value\n\n//code";

    let expected = Option::Some(Output::Text(String::from("Bypassing script no. 1 (! applied)")));
    let obtained = inputs_parse(Inputs {i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_inputs_returns_for_tag_data_absent_some_output_text() {

    let (config_default, i, _, _, _) = get_values_for_parse_inputs();
    let script_plus_tag_line_part = "\n\n//code";

    let expected = Option::Some(Output::Text(String::from("No tag data found for script no. 1")));
    let obtained = inputs_parse(Inputs { i, srcstr: script_plus_tag_line_part, config: &config_default });

    assert_eq!(expected, obtained);
  }
}
