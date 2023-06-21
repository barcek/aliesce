/*
  STRUCTURE

    modules

      main / SOURCE PROCESSING
      - imports
      - SETTINGS, incl. DEFAULTS, CLI OPTIONS
      - OVERVIEW, incl. DOC LINES, MAIN
      - data structures
        - configuration (Config etc.)
        - consolidation (Source, Script, Output etc.)
      - utility functions, remaining
      - primary functions, remaining
        - general
        - output handlers
        - argument applicators

      args / ARGUMENT HANDLING
      - imports
      - data structures
      - argument applicators ('version', 'help')
      - utility functions
      - primary functions, remaining

      test / UNIT TESTING
      - imports
      - test cases
*/

/* main / SOURCE PROCESSING */

/* - imports */

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time::{ Duration, SystemTime };
use std::env;
use std::path::Path;
use std::fs;
use std::process;
use std::collections::HashMap;

use crate::args::{ CLIOption, config_update };

/* - SETTINGS, incl. DEFAULTS, CLI OPTIONS */

static DEFAULTS: [(&str, &str); 10] = [
  ("path_src",     "src.txt"     ), /* source file path (incl. output stem) */
  ("path_dir",     "scripts"     ), /* output directory name */
  ("path_tmp_dir",".aliesce_tmp" ), /* source backup directory name, present during write to source */
  ("tag_head",     "###"         ),
  ("tag_tail",     "#"           ),
  ("sig_stop",     "!"           ),
  ("plc_path_dir", ">"           ),
  ("plc_path_all", ">{}<"        ), /* '{}' is optional script no. position */
  ("cmd_prog",     "bash"        ),
  ("cmd_flag",     "-c"          )
];

fn cli_options_get(config: &Config) -> Vec<CLIOption> {
  Vec::from([
    CLIOption::new("list", "l", &[], &*format!("print for each script in SOURCE (def. '{}') its number and tag line content, without saving or running", config.defaults.get("path_src").unwrap()), &cli_option_list_apply),
    CLIOption::new("only", "o", &["SUBSET"], "include only the scripts the numbers of which appear in SUBSET, comma-separated and/or as ranges, e.g. -o 1,3-5", &cli_option_only_apply),
    CLIOption::new("dest", "d", &["DIRNAME"], &*format!("set the default output dirname ('{}') to DIRNAME", config.defaults.get("path_dir").unwrap()), &cli_option_dest_apply),
    CLIOption::new("init", "i", &[], &*format!("add a source at the default path ('{}') then exit", config.defaults.get("path_src").unwrap()), &cli_option_init_apply),
    CLIOption::new("push", "p", &["LINE", "PATH"], &*format!("append to SOURCE (def. '{}') LINE, auto-prefixed by the tag head, followed by the content at PATH then exit", config.defaults.get("path_src").unwrap()), &cli_option_push_apply),
    CLIOption::new("edit", "e", &["N", "LINE"], &*format!("update the tag line for script number N to LINE, auto-prefixed by the tag head, then exit"), &cli_option_edit_apply),
    CLIOption::new_version(),
    CLIOption::new_help()
  ])
}

/* - OVERVIEW, incl. DOC LINES, MAIN */

fn doc_lines_get(config: &Config) -> [String; 5] {

  let file = format!("The default source path is '{}'. Each script in the file is preceded by a tag line begun with the tag head ('{}') and an optional label and tail ('{}'):", config.defaults.get("path_src").unwrap(), config.defaults.get("tag_head").unwrap(), config.defaults.get("tag_tail").unwrap());
  let line = format!("{}[ label {}] <OUTPUT EXTENSION / PATH: [[[.../]dirname/]stem.]ext> <COMMAND>", config.defaults.get("tag_head").unwrap(), config.defaults.get("tag_tail").unwrap());

  let data_main = format!("Each script is saved with the default output directory ('{}'), source file stem and OUTPUT EXTENSION, or a PATH overriding stem and/or directory, then the COMMAND is run with the save path appended. The '{}' placeholder can be used in the COMMAND to override path position and have the COMMAND passed to '{} {}'; where a script no. is included ('{}') the save path of that script is applied.", config.defaults.get("path_dir").unwrap(), config.defaults.get("plc_path_all").unwrap().replace("{}", ""), config.defaults.get("cmd_prog").unwrap(), config.defaults.get("cmd_flag").unwrap(), config.defaults.get("plc_path_all").unwrap().replace("{}", "n"));
  let data_plus = format!("The '{}' signal can be used before the EXTENSION etc. to avoid both the save and run stages, or before the COMMAND to avoid run only. The '{}' placeholder can be used in a full PATH to denote the default or overridden output directory name.", config.defaults.get("sig_stop").unwrap(), config.defaults.get("plc_path_dir").unwrap());

  let pipe = format!("One or more file paths can be piped to aliesce to append the content at each to the source as a script, auto-preceded by a tag line with a base '{}', then exit.", config.defaults.get("sig_stop").unwrap());

  [file, line, data_main, data_plus, pipe]
}

fn main() {

  /* INITIAL SETUP */

  let config_init = Config { defaults: HashMap::from(DEFAULTS), receipts: HashMap::new() };
  let cli_options = cli_options_get(&config_init);

  /* update config for args passed to command */
  let args_on_cli = env::args().skip(1).collect::<Vec<String>>();
  let config_base = config_update(config_init, &cli_options, &args_remaining_cli_apply, args_on_cli);

  /* SOURCE APPEND VIA STDIN */

  if_paths_on_stdin_push_then_exit(&config_base);

  /* SOURCE UPDATE VIA ARGS OR PROCESS TO OUTPUT */

  let source = source_get(&config_base);

  /* update config for args passed in source */
  let args_in_src = source.preface.split_whitespace().map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect::<Vec<String>>();
  let config_full = config_update(config_base, &cli_options, &args_remaining_src_apply, args_in_src);

  if_change_in_args_make_then_exit(&source, &config_full);

  /* get outputs and output subset as context */
  let outputs = outputs_get(source, &config_full);
  let context = context_get(&outputs);

  /* print output if text or process if file */
  outputs.iter()
    .for_each(|output| { output_apply(output, &context) })
}

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
  scripts: Vec<Script>
}

struct Script {
  n:    usize,
  line: String,
  body: String
}

impl Script {
  fn new(n: usize, text: String) -> Script {

    let mut lines = text.lines();
    let line = lines.nth(0).unwrap().to_string();
    let body = lines.skip(1).collect::<Vec<&str>>().join("\n");

    Script { n, line, body }
  }
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
  n: usize
}

impl OutputFile {
  fn new(data: Vec<String>, code: String, n: usize, config: &Config) -> OutputFile {

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
      let init = OutputFileInit::Text(OutputText::Stderr(format!("Not running file no. {} (no values)", n)));
      return OutputFile { data, code, path, init, n };
    }
    if data.get(1).unwrap() == defaults.get("sig_stop").unwrap() {
      let init = OutputFileInit::Text(OutputText::Stderr(format!("Not running file no. {} ({} applied)", n, defaults.get("sig_stop").unwrap())));
      return OutputFile { data, code, path, init, n };
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

    OutputFile { data, code, path, init, n }
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

/* - utility functions, remaining */

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

/* - primary functions, remaining */

/*   - general */

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

fn if_change_in_args_make_then_exit(source: &Source, config: &Config) {

  let args = match config.receipts.get("edit") {
    Some(ConfigRecsVal::Strs(strs)) => strs.to_owned(),
    _                               => Vec::new()
  };

  /* handle source changes for any args */
  if !args.is_empty() {

    let arg_n = args[0].parse::<usize>().expect("parse no. for option 'edit'");
    let arg_line = &args[1];

    /* update tag line and join whole */
    let source_scripts = source.scripts.iter()
      .map(|script| {
        let Script { n, line, body } = script;
        format!("{}{}\n\n{}\n", config.defaults.get("tag_head").unwrap(), if arg_n == *n { format!(" {}", arg_line) } else { line.to_string() }, body)
      })
      .collect::<String>();

    let text = format!("{}{}", source.preface, source_scripts);

    /* write source to file, with backup to then removal of temporary directory */
    let path_src = config.get_path_src();
    let path_src_inst = Path::new(&path_src);
    let path_src_stem = path_src_inst.file_stem().unwrap().to_str().unwrap();
    let path_src_ext  = path_src_inst.extension().unwrap().to_str().unwrap();

    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

    let path_tmp_dir = config.defaults.get("path_tmp_dir").unwrap();
    let path_tmp = format!("{}/{}_{}.{}", path_tmp_dir, path_src_stem, secs, path_src_ext);

    fs::create_dir_all(&path_tmp_dir)
      .unwrap_or_else(|_| panic!("create temporary directory '{}' for source backup", &path_tmp_dir));
    fs::copy(&path_src, &path_tmp)
      .unwrap_or_else(|_| panic!("copy source as backup to '{}'", &path_tmp));
    fs::write(&path_src, text)
      .unwrap_or_else(|_| panic!("write updated source to '{}'", &path_src));
    fs::remove_dir_all(&path_tmp_dir)
      .unwrap_or_else(|_| panic!("remove temporary directory '{}'", &path_tmp_dir));

    println!("Updated tag line for script no. {} to '{}'", arg_n, arg_line);
    process::exit(0);
  };
}

fn source_get(config: &Config) -> Source {

  let [doc_line_file, doc_line_line, _, _, _] = &doc_lines_get(&config);

  /* load source file content as string or exit early */
  let sections = fs::read_to_string(&config.get_path_src())
    .unwrap_or_else(|err| error_handle((&format!("Not parsing source file '{}'", config.get_path_src()), Some("read"), Some(err))))
    /* set any init option text with tag head to placeholder */
    .lines()
    .map(|line| if doc_line_file == line { "plc_doc_line_file" } else { line })
    .map(|line| if doc_line_line == line { "plc_doc_line_line" } else { line })
    .collect::<Vec<&str>>().join("\n")
    /* get args section plus each source string (script with tag line minus tag head) numbered */
    .split(config.defaults.get("tag_head").unwrap()).map(|part| part.to_owned())
    .enumerate()
    /* remove any shebang line */
    .map(|(i, part)| if 0 == i && part.len() >= 2 && "#!" == &part[..2] { (i, part.splitn(2, '\n').last().unwrap().to_string()) } else { (i, part) })
    .collect::<Vec<(usize, String)>>();

  let preface = sections[0].1
    /* restore any init option text set to placeholder */
    .replace("plc_doc_line_file", doc_line_file)
    .replace("plc_doc_line_line", doc_line_line);
  let scripts = Vec::from(sections.split_at(1).1).iter().map(|section| Script::new(section.0, section.1.to_owned())).collect::<Vec<Script>>();

  Source { preface, scripts }
}

fn inputs_parse(script: &Script, config: &Config) -> Output {

  let Script { n, line, body } = script;
  let Config { defaults, receipts } = config;

  /* get label and data from tag line */
  let line_sections = match line.find(defaults.get("tag_tail").unwrap()) {
    Some(i) => line.split_at(i + 1),
    None    => ("", line.as_str())
  };
  let line_label = line_sections.0.split(defaults.get("tag_tail").unwrap()).nth(0).unwrap(); /* untrimmed */
  let line_data  = line_sections.1.trim();

  /* handle option - list - print only */
  if receipts.contains_key("list") {
    let join = if !line_label.is_empty() { [line_label, ":"].concat() } else { "".to_string() };
    let text = format!("{}:{} {}", n, join, line_data);
    return Output::Text(OutputText::Stdout(text));
  };

  /* get items from tag line data */
  let data = line_data.split(' ')
    .map(|item| item.to_string())
    .filter(|item| !item.is_empty()) /* remove whitespace */
    .collect::<Vec<String>>();

  /* handle data absent or bypass */
  if data.is_empty() {
    let text = format!("No tag data found for script no. {}", n);
    return Output::Text(OutputText::Stderr(text));
  }
  if data.get(0).unwrap() == defaults.get("sig_stop").unwrap() {
    let text = format!("Bypassing script no. {} ({} applied)", n, defaults.get("sig_stop").unwrap());
    return Output::Text(OutputText::Stderr(text));
  }

  Output::File(OutputFile::new(data, body.to_owned(), n.to_owned(), config))
}

fn outputs_get(source: Source, config: &Config) -> Vec<Output> {
  source.scripts.iter()
    /* handle option - only - allow subset */
    .filter(|script| !config.receipts.contains_key("only") || match config.receipts.get("only").unwrap() {
      ConfigRecsVal::Ints(val_ints) => val_ints.contains(&script.n),
      _                             => false
    })
    /* parse input set to output instance */
    .map(|script| inputs_parse(script, &config))
    .collect::<Vec<Output>>()
}

fn context_get(outputs: &Vec<Output>) -> HashMap<usize, String> {
  outputs.iter()
    /* get each output path with script no. */
    .fold(HashMap::new(), |mut acc: HashMap<usize, String>, output| {
      if let Output::File(file) = output { acc.insert(file.n, file.path.get()); }
      acc
    })
}

/*   - output handlers */

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

  let OutputFile { data: _, code, path, init: _, n: _ } = output;
  let dir = &path.dir;
  let path = path.get();

  /* add directory if none */
  fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("create directory '{}'", &dir));
  /* write script to file */
  fs::write(&path, code).unwrap_or_else(|_| panic!("write script to '{}'", &path));
}

fn output_exec(output: &OutputFile, context: &HashMap<usize, String>) {

  let OutputFile { data: _, code: _, path: _, init, n } = output;

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
          let path = if 0 == plc.0 { context.get(n).unwrap() } else { context.get(&(plc.0 as usize)).unwrap() };
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

/*   - argument applicators */

fn cli_option_dest_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigRecsVal {
  ConfigRecsVal::Strs(strs)
}

fn cli_option_edit_apply(_0: &Config, _1: &[CLIOption], strs: Vec<String>) -> ConfigRecsVal {
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

  let [file, line, data_main, data_plus, pipe] = doc_lines_get(&config);
  let src = &config.defaults.get("path_src").unwrap();

  let content = format!("\
    <any arguments to aliesce (run 'aliesce --help' for options)>\n\n\
    Notes on source file format:\n\n\
    {}\n\n{}\n\n{}\n\n\
    Appending scripts via stdin:\n\n\
    {}\n\n\
    Tag line and script section:\n\n\
    {}\n\n<script>\n\
    ", file, data_main, data_plus, pipe, line
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

/* args / ARGUMENT HANDLING */

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
    println!("{}", name_and_version_get());
    process::exit(0);
  }

  fn cli_option_help_apply(config: &Config, cli_options: &[CLIOption], _0: Vec<String>) -> ConfigRecsVal {

    let line_length_max = 80;

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

    /* generate title line */
    let title_line = format!("{}", line_center_with_fill(&name_and_version_get(), line_length_max, "-"));

    /* generate usage text */
    let usage_opts_part = cli_options.iter()
      .filter(|cli_option| cli_option.word != "version" && cli_option.word != "help") /* avoid duplication */
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| format!("[--{}/-{}{}]", cli_option.word, cli_option.char, if strs_strs[i].is_empty() { "".to_owned() } else { " ".to_owned() + &strs_strs[i] }))
      .collect::<Vec<String>>()
      .join(" ");
    let usage_opts_head = line_break_and_indent(&format!("{} [SOURCE]", usage_opts_part), 15, line_length_max, false);
    let usage_opts_tail = line_break_and_indent(&format!("/ --version/-v / --help/-h"), 15, line_length_max, true);
    let usage_text = format!("Usage: aliesce {}\n{}", usage_opts_head, usage_opts_tail);

    /* generate flags text */
    let flags_list = cli_options.iter()
      .enumerate() /* yield also index (i) */
      .map(|(i, cli_option)| {
        let desc = line_break_and_indent(&cli_option.desc, flag_strs_max + strs_strs_max + 2, line_length_max, false);
        format!(" {}  {:w$}  {}", flag_strs[i], strs_strs[i], desc, w = flag_strs_max - cli_option.word.len())
      })
      .collect::<Vec<String>>()
      .join("\n");
    let flags_text = format!("Flags:\n{}", flags_list);

    /* generate notes text */
    let notes_body = doc_lines_get(&config).map(|line| line_break_and_indent(&line, 1, line_length_max, true)).join("\n\n");
    let notes_text = format!("Notes:\n{}", notes_body);

    println!("{}\n\n{}\n{}\n\n{}", title_line, usage_text, flags_text, notes_text);
    process::exit(0);
  }

  /* - utility functions */

  fn name_and_version_get() -> String {
    format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
  }

  fn line_center_with_fill(line: &str, length: usize, fill: &str) -> String {
    let whitespace_half = String::from(fill).repeat((length - line.len() - 2) / 2);
    let whitespace_last = if 0 == line.len() % 2 { "" } else { fill };
    format!("{} {} {}{}", whitespace_half, line, whitespace_half, whitespace_last)
  }

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

  /* - primary functions, remaining */

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

/* test / UNIT TESTING */

#[cfg(test)]
mod test {

  /* - imports */

  use::std::collections::HashMap;
  use super::{
    DEFAULTS,
    Config, ConfigRecsVal,
    Script,
    Output, OutputText, OutputFile, OutputFilePath, OutputFileInit, OutputFileInitCode,
    inputs_parse
  };

  /* - test cases */

  /*   - function: inputs_parse */

  fn get_values_for_inputs_parse() -> (Config<'static>, String, usize, String, OutputFilePath, OutputFileInit) {

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

    let body = "//code".to_string();

    let number = 1;
    let prog  = String::from("program");
    let args  = Vec::from([String::from("--flag"), String::from("value"), output_path.get()]);
    let plcs  = Vec::new();
    let code  = String::from("//code");

    let output_init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    (config_default, body, number, code, output_path, output_init)
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_some_output() {

    let (config_default, body, n, code, path, init) = get_values_for_inputs_parse();

    let line = " ext program --flag value\n".to_string();
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, body, n, code, path, init) = get_values_for_inputs_parse();

    let line = " label # ext program --flag value\n".to_string();
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_dest_option_some_output_file() {

    let (mut config_default, body, n, code, _, mut init) = get_values_for_inputs_parse();

    let line = " ext program --flag value\n".to_string();
    let data = Vec::from(["ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dest");
    let stem = String::from(config_default.defaults.get("path_src").unwrap().split(".").nth(0).unwrap());
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };
    config_default.receipts.insert("dest".to_string(), ConfigRecsVal::Strs(Vec::from([String::from("dest")])));

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_list_option_some_output_text() {

    let (mut config_default, body, n, _, _, _) = get_values_for_inputs_parse();

    let line = " ext program --flag value\n".to_string();

    config_default.receipts.insert("list".to_string(), ConfigRecsVal::Bool);

    let expected = Output::Text(OutputText::Stdout(String::from("1: ext program --flag value")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_singlepart_output_stem_some_output_file() {

    let (config_default, body, n, code, _, mut init) = get_values_for_inputs_parse();

    let line = " script.ext program --flag value\n".to_string();
    let data = Vec::from(["script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.defaults.get("path_dir").unwrap().to_owned());
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_multipart_output_stem_some_output_file() {

    let (config_default, body, n, code, _, mut init) = get_values_for_inputs_parse();

    let line = " script.suffix1.suffix2.ext program --flag value\n".to_string();
    let data = Vec::from(["script.suffix1.suffix2.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from(config_default.defaults.get("path_dir").unwrap().to_owned());
    let stem = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_dir_some_output_file() {

    let (config_default, body, n, code, _, mut init) = get_values_for_inputs_parse();

    let line = " dir/script.ext program --flag value\n".to_string();
    let data = Vec::from(["dir/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("dir");
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_dir_placeholder_some_output_file() {

    let (config_default, body, n, code, _, mut init) = get_values_for_inputs_parse();

    let line = " >/script.ext program --flag value\n".to_string();
    let data = Vec::from([">/script.ext".to_string(), "program".to_string(), "--flag".to_string(), "value".to_string()]);

    let dir = String::from("scripts");
    let stem = String::from("script");
    let ext = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_all_placeholder_some_output() {

    let (config_default, body, n, code, path, _) = get_values_for_inputs_parse();

    let line = " ext program_1 --flag value >< | program_2\n".to_string();
    let data = Vec::from(["ext".to_string(), "program_1".to_string(), "--flag".to_string(), "value".to_string(), "><".to_string(), "|".to_string(), "program_2".to_string()]);

    let prog = String::from(config_default.defaults.get("cmd_prog").unwrap().to_owned());
    let args = Vec::from([String::from(config_default.defaults.get("cmd_flag").unwrap().to_owned()), String::from("program_1 --flag value >< | program_2")]);
    let plcs = Vec::from([(0, String::from("><"))]);
    let init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_minus_cmd_some_output_file_indicating() {

    let (config_default, body, n, code, path, _) = get_values_for_inputs_parse();

    let line = " ext\n".to_string();
    let data = Vec::from(["ext".to_string()]);

    let init = OutputFileInit::Text(OutputText::Stderr(String::from("Not running file no. 1 (no values)")));

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_with_bypass_some_output_text() {

    let (config_default, body, n, _, _, _) = get_values_for_inputs_parse();

    let line = " ! ext program --flag value\n".to_string();

    let expected = Output::Text(OutputText::Stderr(String::from("Bypassing script no. 1 (! applied)")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_absent_some_output_text() {

    let (config_default, body, n, _, _, _) = get_values_for_inputs_parse();

    let line = "\n".to_string();

    let expected = Output::Text(OutputText::Stderr(String::from("No tag data found for script no. 1")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }
}
