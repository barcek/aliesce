/*
  SOURCE PROCESSING
  - imports
  - DEFAULT VALUES, general
  - data structures
    - configuration (Config etc.)
    - consolidation (Inputs, Output etc.)
  - utility functions, incl. DOC LINES
  - primary functions, incl. MAIN (w/ CLI OPTIONS)
  - argument applicators

  ARGUMENT HANDLING / mod args
  - imports
  - data structures
  - argument applicators ('version', 'help')
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

/* - DEFAULT VALUES, general */

static DEFAULTS: [(&str, &str); 9] = [
  ("path_src",     "src.txt"), /* source file path (incl. output stem) */
  ("path_dir",     "scripts"), /* output directory name */
  ("tag_head",     "###"    ),
  ("tag_tail",     "#"      ),
  ("sig_stop",     "!"      ),
  ("plc_path_dir", ">"      ),
  ("plc_path_all", ">{}<"   ), /* '{}' is optional script no. position */
  ("cmd_prog",     "bash"   ),
  ("cmd_flag",     "-c"     )
];

/* - data structures */

/*   - configuration */

pub struct Config<'a> {
  defaults: HashMap<&'a str, &'a str>,
  receipts: HashMap<String, ConfigRecsVal>
}

impl Config<'_> {
  /* handle any positional argument - alternative source file path */
  fn get_path_src(&self) -> String {
    if self.receipts.contains_key("path_src") {
      if let ConfigRecsVal::Strs(val_strs) = self.receipts.get("path_src").unwrap() {
        return val_strs.get(0).unwrap().to_string();
      }
    }
    String::from(self.defaults.get("path_src").unwrap().to_owned())
  }
  /* handle option - dest - alternative output directory name */
  fn get_path_dir(&self) -> String {
    if self.receipts.contains_key("dest") {
      if let ConfigRecsVal::Strs(val_strs) = self.receipts.get("dest").unwrap() {
        return val_strs.get(0).unwrap().to_string();
      }
    }
    String::from(self.defaults.get("path_dir").unwrap().to_owned())
  }
}

#[derive(PartialEq, Eq)]
pub enum ConfigRecsVal {
  Bool,
  Ints(Vec<usize>),
  Strs(Vec<String>)
}

/*   - consolidation */

struct Source {
  preface: String,
  scripts: Vec<(usize, String)>
}

struct Inputs<'a> {
  script: (usize, String),
  config: &'a Config<'a>
}

#[derive(Debug, PartialEq)]
enum Output {
  Text(OutputText),
  File(OutputFile)
}

#[derive(Debug, PartialEq)]
enum OutputText {
  Stdout(String),
  Stderr(String)
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

    let Config { defaults, receipts: _ } = config;

    /* set output path parts */

    /* get output path parts - break first data item on '/' */
    let mut parts_path = data.get(0).unwrap().split('/').collect::<Vec<&str>>();
    let path_dir = config.get_path_dir();

    /* handle output directory identified by directory placeholder */
    if defaults.get("plc_path_dir").unwrap() == &parts_path[0] { parts_path[0] = path_dir.as_str() };

    /* get output filename parts - separate last output path part and break on '.' */
    let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split('.').collect::<Vec<&str>>();
    let p_f_len = parts_filename.len();

    /* set as dir either remaining output path parts recombined or directory name,
           as stem either all but last output filename part or src stem, and
           as ext last output filename part */
    let dir = if !parts_path.is_empty() { parts_path.join("/") } else { path_dir.to_string() };
    let stem = if p_f_len > 1 { parts_filename[..(p_f_len - 1)].join(".") } else { config.get_path_src().split('.').nth(0).unwrap().to_string() };
    let ext = parts_filename.iter().last().unwrap().to_string();

    let path = OutputFilePath{ dir, stem, ext };

    /* set output init parts */

    /* handle file run precluded */
    if data.len() == 1 {
      let init = OutputFileInit::Text(OutputText::Stderr(format!("Not running file no. {} (no values)", i)));
      return OutputFile { data, code, path, init, i };
    }
    if data.get(1).unwrap() == defaults.get("sig_stop").unwrap() {
      let init = OutputFileInit::Text(OutputText::Stderr(format!("Not running file no. {} ({} applied)", i, defaults.get("sig_stop").unwrap())));
      return OutputFile { data, code, path, init, i };
    }

    /* set as plcs any uses of output path placeholder and note presence as indicator of composite command */
    let mut parts_placeholder = defaults.get("plc_path_all").unwrap().split("{}");
    let plc_head = parts_placeholder.next().unwrap();
    let plc_tail = parts_placeholder.next().unwrap();
    let plc_full = Vec::from([plc_head, plc_tail]).join("");

    let plcs = data.iter().skip(1).map(|item| {
      /* handle request for current script output path */
      if item.contains(&plc_full) { return (0, plc_full.to_owned()) };
      let head_i = if let Some(i) = item.find(plc_head) { i as i8 } else { -1 };
      let tail_i = if let Some(i) = item.find(plc_tail) { i as i8 } else { -1 };
      /* handle request for another script output path */
      if -1 != head_i && -1 != tail_i && head_i < tail_i {
         let s = item.chars().skip(head_i as usize).take((tail_i - head_i + 1) as usize).collect::<String>();
         let i = s.chars().skip(plc_head.len()).take(s.len() - plc_full.len()).collect::<String>().parse::<i8>().unwrap();
         return (i, s)
      };
      /* handle no request - value to be filtered out */
      (-1, String::new())
    }).filter(|item| -1 != item.0).collect::<Vec<(i8, String)>>();

    let has_placeholder = !plcs.is_empty();

    /* set as prog either tag line second item or default, and
           as args either Vec containing remaining items plus combined path or default flag plus remaining items joined */
    let prog = if has_placeholder { String::from(defaults.get("cmd_prog").unwrap().to_owned()) } else { data.get(1).unwrap().to_owned() };
    let args = if has_placeholder {
      Vec::from([defaults.get("cmd_flag").unwrap().to_string(), data.iter().skip(1).map(|item| item.to_owned()).collect::<Vec<String>>().join(" ")])
    }
    else {
      [data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>(), Vec::from([path.get()])].concat()
    };

    let init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    OutputFile { data, code, path, init, i }
  }
}

#[derive(Debug, PartialEq)]
struct OutputFilePath {
  dir: String,
  stem: String,
  ext: String
}

impl OutputFilePath {
  fn get(&self) -> String {
    format!("{}/{}.{}", &self.dir, &self.stem, &self.ext)
  }
}

#[derive(Debug, PartialEq)]
enum OutputFileInit {
  Text(OutputText),
  Code(OutputFileInitCode)
}

#[derive(Debug, PartialEq)]
struct OutputFileInitCode {
  prog: String,
  args: Vec<String>,
  plcs: Vec<(i8, String)>
}

/* - utility functions, incl. DOC LINES */

fn doc_lines_get(config: &Config) -> [String; 5] {

  let form = format!("The default source path is '{}'. Each script in the file is preceded by a tag line begun with the tag head ('{}') and an optional label and tail ('{}'):", config.defaults.get("path_src").unwrap(), config.defaults.get("tag_head").unwrap(), config.defaults.get("tag_tail").unwrap());
  let line = format!("{}[ label {}] <OUTPUT EXTENSION / PATH: [[[.../]dirname/]stem.]ext> <COMMAND>", config.defaults.get("tag_head").unwrap(), config.defaults.get("tag_tail").unwrap());

  let data_items = format!("Each script is saved with the default output directory ('{}'), source file stem and OUTPUT EXTENSION, or a PATH overriding stem and/or directory, then the COMMAND is run with the save path appended. The '{}' placeholder can be used in the COMMAND to override path position and have the COMMAND passed to '{} {}'; where a script no. is included ('{}') the save path of that script is applied.", config.defaults.get("path_dir").unwrap(), config.defaults.get("plc_path_all").unwrap().replace("{}", ""), config.defaults.get("cmd_prog").unwrap(), config.defaults.get("cmd_flag").unwrap(), config.defaults.get("plc_path_all").unwrap().replace("{}", "n"));
  let data_chars = format!("The '{}' signal can be used before the EXTENSION etc. to avoid both the save and run stages, or before the COMMAND to avoid run only. The '{}' placeholder can be used in a full PATH to denote the default or overridden output directory name.", config.defaults.get("sig_stop").unwrap(), config.defaults.get("plc_path_dir").unwrap());

  let read = format!("One or more file paths can be piped to aliesce to append the content at each to the source as a script, auto-preceded by a tag line with a base '{}', then exit.", config.defaults.get("sig_stop").unwrap());

  [form, line, data_items, data_chars, read]
}

fn script_push(config: &Config, strs: Vec<String>) {

  let script_filename = &strs[1];
  let Config { defaults, receipts: _ } = config;

  /* handle read */

  let script = fs::read_to_string(script_filename)
    .unwrap_or_else(|err| error_handle((&format!("Not parsing script file '{}'", script_filename), Some("read"), Some(err))));
  let tag_line = format!("{} {}", defaults.get("tag_head").unwrap(), strs[0]);
  let script_plus_tag_line = format!("\n{}\n\n{}", tag_line, script);

  /* handle write */

  use io::Write;
  let sum_base = format!("tag line '{}' and content of script file '{}' to source file '{}'", tag_line, script_filename, config.get_path_src());
  let sum_failure = format!("Not appending {}", sum_base);
  let sum_success = format!("Appended {}", sum_base);

  let mut file = fs::OpenOptions::new().append(true).open(config.get_path_src())
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

  let config_init = Config { defaults: HashMap::from(DEFAULTS), receipts: HashMap::new() };
  let cli_options = cli_options_get(&config_init);

  /* update config for args passed to command */
  let args_on_cli = env::args().skip(1).collect::<Vec<String>>();
  let config_base = config_update(config_init, &cli_options, &args_remaining_cli_apply, args_on_cli);

  /* handle reads from stdin and source path */
  if_paths_on_stdin_push_then_exit(&config_base);
  let source = source_get(&config_base);

  /* update config for args passed in source */
  let args_in_src = source.preface.split_whitespace().map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect::<Vec<String>>();
  let config_full = config_update(config_base, &cli_options, &args_remaining_src_apply, args_in_src);

  /* get outputs and output subset as context */
  let outputs = outputs_get(source, &config_full);
  let context = context_get(&outputs);

  /* print output if text or process if file */
  outputs.iter()
    .for_each(|output| { output_apply(output, &context) })
}

fn cli_options_get(config: &Config) -> Vec<CLIOption> {
  Vec::from([
    CLIOption::new("dest", "d", &["DIRNAME"], &*format!("set the default output dirname ('{}') to DIRNAME", config.defaults.get("path_dir").unwrap()), &cli_option_dest_apply),
    CLIOption::new("list", "l", &[], &*format!("print for each script in the source ('{}') its number and tag line content, without saving or running", config.defaults.get("path_src").unwrap()), &cli_option_list_apply),
    CLIOption::new("only", "o", &["SUBSET"], "include only the scripts the numbers of which appear in SUBSET, comma-separated and/or as ranges, e.g. -o 1,3-5", &cli_option_only_apply),
    CLIOption::new("push", "p", &["LINE", "PATH"], &*format!("append to the source ('{}') LINE, auto-prefixed by the tag head, followed by the content at PATH then exit", config.defaults.get("path_src").unwrap()), &cli_option_push_apply),
    CLIOption::new("init", "i", &[], &*format!("add a source at the default path ('{}') then exit", config.defaults.get("path_src").unwrap()), &cli_option_init_apply),
    CLIOption::new_version(),
    CLIOption::new_help()
  ])
}

fn if_paths_on_stdin_push_then_exit(config: &Config) {

  use io::Read;
  let (tx, rx) = mpsc::channel();

  /* spawn thread for blocking read and send string */
  thread::spawn(move || {
    let mut stdin = io::stdin();
    let mut bfr = String::new();
    stdin.read_to_string(&mut bfr).unwrap();
    tx.send(bfr).unwrap();
  });
  thread::sleep(Duration::from_millis(25));

  /* process lines in string received to paths */
  let paths = match rx.try_recv() {
    Ok(recvd) => recvd.split_whitespace().map(|str| str.to_string()).filter(|str| !str.is_empty()).collect::<Vec<String>>(),
    Err(_)    => Vec::new()
  };

  /* handle script pushes for any paths */
  if !paths.is_empty() {
    for path in paths {
      script_push(&config, Vec::from([config.defaults.get("sig_stop").unwrap().to_string(), path]));
    }
    process::exit(0);
  };
}

fn source_get(config: &Config) -> Source {

  let [form, line, _, _, _] = &doc_lines_get(&config);

  /* load source file content as string or exit early */
  let sections = fs::read_to_string(&config.get_path_src())
    .unwrap_or_else(|err| error_handle((&format!("Not parsing source file '{}'", config.get_path_src()), Some("read"), Some(err))))
    /* remove any init option text tag heads */
    .lines()
    .map(|ln| if form == ln || line == ln { "" } else { ln })
    .collect::<Vec<&str>>().join("\n")
    /* get args section plus each source string (script with tag line minus tag head) numbered */
    .split(config.defaults.get("tag_head").unwrap()).map(|part| part.to_owned())
    .enumerate()
    /* remove any shebang line */
    .map(|(i, part)| if 0 == i && part.len() >= 2 && "#!" == &part[..2] { (i, part.splitn(2, '\n').last().unwrap().to_string()) } else { (i, part) })
    .collect::<Vec<(usize, String)>>();

  let preface = String::from(&sections[0].1);
  let scripts = Vec::from(sections.split_at(1).1);

  Source { preface, scripts }
}

fn inputs_parse(inputs: Inputs) -> Output {

  let Inputs { script, config } = inputs;
  let (number, srcstr) = script;
  let Config { defaults, receipts } = config;

  let mut lines = srcstr.lines();
  let tag_line_part = lines.nth(0).unwrap();

  /* get label and data from tag line */
  let tag_line_sections = match tag_line_part.find(defaults.get("tag_tail").unwrap()) {
    Some(i) => tag_line_part.split_at(i + 1),
    None    => ("", tag_line_part)
  };
  let tag_line_label = tag_line_sections.0.split(defaults.get("tag_tail").unwrap()).nth(0).unwrap(); /* untrimmed */
  let tag_line_data  = tag_line_sections.1.trim();

  /* handle option - list - print only */
  if receipts.contains_key("list") {
    let join = if !tag_line_label.is_empty() { [tag_line_label, ":"].concat() } else { "".to_string() };
    let text = format!("{}:{} {}", number, join, tag_line_data);
    return Output::Text(OutputText::Stdout(text));
  };

  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");

  /* get items from tag line data */
  let data = tag_line_data.split(' ')
    .map(|item| item.to_string())
    .filter(|item| !item.is_empty()) /* remove whitespace */
    .collect::<Vec<String>>();

  /* handle data absent or bypass */
  if data.is_empty() {
    let text = format!("No tag data found for script no. {}", number);
    return Output::Text(OutputText::Stderr(text));
  }
  if data.get(0).unwrap() == defaults.get("sig_stop").unwrap() {
    let text = format!("Bypassing script no. {} ({} applied)", number, defaults.get("sig_stop").unwrap());
    return Output::Text(OutputText::Stderr(text));
  }

  Output::File(OutputFile::new(data, code, number, config))
}

fn outputs_get(source: Source, config: &Config) -> Vec<Output> {
  source.scripts.iter()
    /* process each part to input instance */
    .map(|script| Inputs { script: script.to_owned(), config })
    /* handle option - only - allow subset */
    .filter(|inputs| !inputs.config.receipts.contains_key("only") || match inputs.config.receipts.get("only").unwrap() {
      ConfigRecsVal::Ints(val_ints) => val_ints.contains(&(inputs.script.0)),
      _                            => false
    })
    /* parse each input to output instance */
    .map(inputs_parse)
    .collect::<Vec<Output>>()
}

fn context_get(outputs: &Vec<Output>) -> HashMap<usize, String> {
  outputs.iter()
    /* get each output path with script no. */
    .fold(HashMap::new(), |mut acc: HashMap<usize, String>, output| {
      if let Output::File(file) = output { acc.insert(file.i, file.path.get()); }
      acc
    })
}

fn output_apply(output: &Output, context: &HashMap<usize, String>) {
  match output {
    Output::Text(e) => {
      match e {
        OutputText::Stdout(s) => {  println!("{}", &s); },
        OutputText::Stderr(s) => { eprintln!("{}", &s); }
      }
    },
    Output::File(s) => { output_save(&s); output_exec(&s, &context); },
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

fn output_exec(output: &OutputFile, context: &HashMap<usize, String>) {

  let OutputFile { data: _, code: _, path: _, init, i } = output;

  match init {

    /* print reason file run precluded */
    OutputFileInit::Text(e) => {
      match e {
        OutputText::Stdout(s) => {  println!("{}", &s); },
        OutputText::Stderr(s) => { eprintln!("{}", &s); }
      }
    },
    /* run script from file */
    OutputFileInit::Code(c) => {
      let OutputFileInitCode { prog, args, plcs } = c;

      let args_full = if plcs.is_empty() {
        args.to_owned()
      } else {
        let mut cmd = if 0 == plcs.len() { String::new() } else { args[1].to_owned() };
        plcs.iter().for_each(|plc| {
          let path = if 0 == plc.0 { context.get(i).unwrap() } else { context.get(&(plc.0 as usize)).unwrap() };
          cmd = cmd.replace(plc.1.as_str(), path.as_str()).to_owned();
        });
        Vec::from([args[0].to_owned(), cmd])
      };

      process::Command::new(&prog).args(args_full)
        .spawn().unwrap_or_else(|_| panic!("run file with '{}'", prog))
        .wait_with_output().unwrap_or_else(|_| panic!("await output from '{}'", prog));
    }
  }
}

/* - argument applicators */

fn cli_option_dest_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigRecsVal {
  ConfigRecsVal::Strs(strs)
}

fn cli_option_list_apply(_0: &Config, _1: &[CLIOption], _2: Vec<String>) -> ConfigRecsVal {
  ConfigRecsVal::Bool
}

fn cli_option_only_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigRecsVal {
  let val_ints: Vec<usize> = strs[0].trim().split(',')
    .flat_map(|val_str| {
      let vals: Vec<usize> = val_str.trim().split('-').map(|item| item.parse::<usize>().expect("parse subset for option 'only'")).collect();
      if vals.len() > 1 { (vals[0]..(vals[1] + 1)).collect::<Vec<usize>>() } else { vals }
    })
    .collect();
  ConfigRecsVal::Ints(val_ints)
}

fn cli_option_push_apply(config: &Config, _0: &[CLIOption], strs: Vec<String>) -> ConfigRecsVal {
  script_push(config, strs);
  process::exit(0);
}

fn cli_option_init_apply(config: &Config, _0: &[CLIOption], _1: Vec<String>) -> ConfigRecsVal {

  let [form, line, data_items, data_chars, read] = doc_lines_get(&config);
  let src = &config.defaults.get("path_src").unwrap();

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
  /* set final source filename (incl. output stem) per positional arg */
  let arg = if !args_remaining.is_empty() { args_remaining.get(0).unwrap().to_owned() } else { String::from(config.defaults.get("path_src").unwrap().to_owned()) };
  let val = ConfigRecsVal::Strs(Vec::from([arg]));
  config.receipts.insert(String::from("path_src"), val);
  config
}

fn args_remaining_src_apply(config: Config, _: Vec<String>) -> Config {
  config
}

/* ARGUMENT HANDLING */

mod args {

  /* - imports */

  use std::process;
  use super::{ Config, ConfigRecsVal, doc_lines_get };

  /* - data structures */

  type CLIOptionCall = dyn Fn(&Config, &[CLIOption], Vec<String>) -> ConfigRecsVal;

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
    pub fn new_version() -> CLIOption {
      CLIOption::new("version", "v", &[], "show name and version number then exit", &cli_option_version_apply)
    }
    pub fn new_help() -> CLIOption {
      CLIOption::new("help", "h", &[], "show usage, flags available and notes then exit", &cli_option_help_apply)
    }
  }

  type CLIArgHandler = dyn Fn(Config, Vec<String>) -> Config;

  /* - argument applicator ('help') */

  fn cli_option_version_apply(_0: &Config, _1: &[CLIOption], _2: Vec<String>) -> ConfigRecsVal {
    println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    process::exit(0);
  }

  fn cli_option_help_apply(config: &Config, cli_options: &[CLIOption], _0: Vec<String>) -> ConfigRecsVal {

    /* set value substrings and max length */
    let strs_strs = cli_options.iter()
      .map(|cli_option| cli_option.strs.join(" "))
      .collect::<Vec<String>>();
    let strs_strs_max = strs_strs.iter()
      .fold(0, |acc, val_str| if val_str.len() > acc { val_str.len() } else { acc });
    let flag_strs = cli_options.iter()
      .map(|cli_option| format!("-{}, --{}", cli_option.char, cli_option.word))
      .collect::<Vec<String>>();
    let flag_strs_max = flag_strs.iter()
      .fold(0, |acc, arg_str| if arg_str.len() > acc { arg_str.len() } else { acc });

    /* generate usage text */
    let usage_opts_part = cli_options.iter()
      .filter(|cli_option| cli_option.word != "help") /* avoid duplication */
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| format!("[--{}/-{}{}]", cli_option.word, cli_option.char, if strs_strs[i].is_empty() { "".to_owned() } else { " ".to_owned() + &strs_strs[i] }))
      .collect::<Vec<String>>()
      .join(" ");
    let usage_opts_full = line_break_and_indent(&format!("[--help/-h / {} [source]]", usage_opts_part), 15, 80, false);
    let usage_text = format!("Usage: aliesce {}", usage_opts_full);

    /* generate flags text */
    let flags_list = cli_options.iter()
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| {
        let desc = line_break_and_indent(&cli_option.desc, flag_strs_max + strs_strs_max + 2, 80, false);
        format!(" {}  {:w$}  {}", flag_strs[i], strs_strs[i], desc, w = flag_strs_max - cli_option.word.len())
      })
      .collect::<Vec<String>>()
      .join("\n");
    let flags_text = format!("Flags:\n{}", flags_list);

    /* generate notes text */
    let notes_body = doc_lines_get(&config).map(|line| line_break_and_indent(&line, 1, 80, true)).join("\n\n");
    let notes_text = format!("Notes:\n{}", notes_body);

    println!("{}\n{}\n\n{}", usage_text, flags_text, notes_text);
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
        config.receipts.insert(word.to_string(), value);
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
    DEFAULTS,
    Config, ConfigRecsVal,
    Inputs,
    Output, OutputText, OutputFile, OutputFilePath, OutputFileInit, OutputFileInitCode,
    inputs_parse
  };

  /* - test cases */

  /*   - function: inputs_parse */

  fn get_values_for_inputs_parse() -> (Config<'static>, usize, String, OutputFilePath, OutputFileInit) {

    let config_default = Config {
      defaults: HashMap::from(DEFAULTS),
      receipts: HashMap::new()
    };

    /* base test script values */

    let output_path = OutputFilePath {
      dir:  String::from(config_default.defaults.get("path_dir").unwrap().to_owned()),
      stem: String::from(config_default.defaults.get("path_src").unwrap().split(".").nth(0).unwrap()),
      ext:  String::from("ext")
    };

    let index = 1;
    let prog  = String::from("program");
    let args  = Vec::from([String::from("--flag"), String::from("value"), output_path.get()]);
    let plcs  = Vec::new();
    let code  = String::from("//code");

    let output_init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    (config_default, index, code, output_path, output_init)
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_some_output() {

    let (config_default, i, code, path, init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code".to_string();
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, i, code, path, init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " label # ext program --flag value\n\n//code".to_string();
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_dest_option_some_output_file() {

    let (mut config_default, i, code, _, mut init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code".to_string();

    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dest");
    let stem = String::from(config_default.defaults.get("path_src").unwrap().split(".").nth(0).unwrap());
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };
    config_default.receipts.insert("dest".to_string(), ConfigRecsVal::Strs(Vec::from([String::from("dest")])));

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_list_option_some_output_text() {

    let (mut config_default, i, _, _, _) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code".to_string();

    config_default.receipts.insert("list".to_string(), ConfigRecsVal::Bool);

    let expected = Output::Text(OutputText::Stdout(String::from("1: ext program --flag value")));
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_singlepart_output_stem_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " script.ext program --flag value\n\n//code".to_string();

    let data = Vec::from(["script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.defaults.get("path_dir").unwrap().to_owned());
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_multipart_output_stem_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " script.suffix1.suffix2.ext program --flag value\n\n//code".to_string();

    let data = Vec::from(["script.suffix1.suffix2.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.defaults.get("path_dir").unwrap().to_owned());
    let stem = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_dir_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " dir/script.ext program --flag value\n\n//code".to_string();

    let data = Vec::from(["dir/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dir");
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_dir_placeholder_some_output_file() {

    let (config_default, i, code, _, mut init) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " >/script.ext program --flag value\n\n//code".to_string();

    let data = Vec::from([">/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("scripts");
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_all_placeholder_some_output() {

    let (config_default, i, code, path, _) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ext program_1 --flag value >< | program_2\n\n//code".to_string();
    let data = Vec::from(["ext".to_string(), "program_1".to_string(), "--flag".to_string(), "value".to_string(), "><".to_string(), "|".to_string(), "program_2".to_string()]);

    let prog = String::from(config_default.defaults.get("cmd_prog").unwrap().to_owned());
    let args = Vec::from([String::from(config_default.defaults.get("cmd_flag").unwrap().to_owned()), String::from("program_1 --flag value >< | program_2")]);
    let plcs = Vec::from([(0, String::from("><"))]);
    let init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_minus_cmd_some_output_file_indicating() {

    let (config_default, i, code, path, _) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ext\n\n//code".to_string();

    let data = Vec::from(["ext".to_string()]);
    let init = OutputFileInit::Text(OutputText::Stderr(String::from("Not running file no. 1 (no values)")));

    let expected = Output::File(OutputFile { data, code, path, init, i });
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_with_bypass_some_output_text() {

    let (config_default, i, _, _, _) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = " ! ext program --flag value\n\n//code".to_string();

    let expected = Output::Text(OutputText::Stderr(String::from("Bypassing script no. 1 (! applied)")));
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_absent_some_output_text() {

    let (config_default, i, _, _, _) = get_values_for_inputs_parse();
    let script_plus_tag_line_part = "\n\n//code".to_string();

    let expected = Output::Text(OutputText::Stderr(String::from("No tag data found for script no. 1")));
    let obtained = inputs_parse(Inputs { script: (i, script_plus_tag_line_part), config: &config_default });

    assert_eq!(expected, obtained);
  }
}
