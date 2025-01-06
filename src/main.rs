/*
  MODULES

  MAIN / source processing
  - imports
  - configuration
    - DEFAULTS
    - settings
    - messages
  - MAIN
  - data structures
    - Source
    - Script
  - primary functions
    - general
    - argument applicators
  - utility functions

  OUTPUT
  - imports
  - data structures
    - Output + components

  CONFIG, incl. argument_handling
  - imports
  - data structures
    - Config + components
  - argument applicators ('version', 'help')
  - utility functions

  TEST
  - imports
  - test cases
    - end-to-end
    - unit
*/

/* MAIN / SOURCE PROCESSING */

/* - imports */

use std::io::{self, Read, Write};
use std::thread;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};
use std::env;
use std::path::Path;
use std::fs;
use std::process;
use std::collections::HashMap;

use crate::output::{
  Output,
  OutputText,
  OutputFile
};
use crate::config::{
  Config,
  ConfigDefaults,
  ConfigSettings,
  ConfigMessages,
  ConfigReceipts,
  ConfigSetting,
  ConfigReceiptVal
};

/* - configuration */

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

fn settings_new(defaults: &ConfigDefaults) -> ConfigSettings {

  Vec::from([

    ConfigSetting::new(
      "list", "l", &[],
      &format!(
        "print for each script in SOURCE (def. '{}') its number and tag line content, without saving or running",
        defaults.get("path_src").expect("get default value 'path_src'")
      ),
      &setting_list_apply
    ),
    ConfigSetting::new(
      "only", "o", &["SUBSET"],
      "include only the scripts the numbers of which appear in SUBSET, comma-separated and/or as ranges, e.g. -o 1,3-5",
      &setting_only_apply
    ),
    ConfigSetting::new(
      "dest", "d", &["DIRNAME"],
      &format!(
        "set the default output dirname ('{}') to DIRNAME",
        defaults.get("path_dir").expect("get default value 'path_dir'")
      ),
      &setting_dest_apply
    ),
    ConfigSetting::new(
      "init", "i", &[],
      &format!(
        "create the source file SOURCE (def. '{}') then exit",
        defaults.get("path_src").expect("get default value 'path_src'")
      ),
      &setting_init_apply
    ),
    ConfigSetting::new(
      "push", "p", &["LINE", "PATH"],
      &format!(
        "append to SOURCE (def. '{}') LINE, adding the tag head if none, followed by the content at PATH then exit",
        defaults.get("path_src").expect("get default value 'path_src'")
      ),
      &setting_push_apply
    ),
    ConfigSetting::new(
      "edit", "e", &["N", "LINE"],
      "update the tag line for script number N to LINE, adding the tag head if none, then exit",
      &setting_edit_apply
    ),
    ConfigSetting::new_version(),
    ConfigSetting::new_help()
  ])
}

fn messages_new(defaults: &ConfigDefaults) -> ConfigMessages<'static> {

  let repository = [
    (
      "file", format!(
        "The default source path is '{}'. Each script in the file is preceded by a tag line begun with the tag head ('{}') and an optional label and tail ('{}'):",
        defaults.get("path_src").expect("get default value 'path_src'"),
        defaults.get("tag_head").expect("get default value 'tag_head'"),
        defaults.get("tag_tail").expect("get default value 'tag_tail'")
      )
    ),
    (
      "line", format!(
        "{}[ label {}] <OUTPUT EXTENSION / PATH: [[[.../]dirname/]stem.]ext> <COMMAND>",
        defaults.get("tag_head").expect("get default value 'tag_head'"),
        defaults.get("tag_tail").expect("get default value 'tag_tail'")
      )
    ),
    (
      "main", format!(
        "Each script is saved with the default output directory ('{}'), source file stem and OUTPUT EXTENSION, or a PATH overriding stem and/or directory, then the COMMAND is run with the save path appended. The '{}' placeholder can be used in the COMMAND to override path position and have the COMMAND passed to '{} {}'; where a script no. is included ('{}') the save path of that script is applied.",
        defaults.get("path_dir").expect("get default value 'path_dir'"),
        defaults.get("plc_path_all").expect("get default value 'plc_path_all'").replace("{}", ""),
        defaults.get("cmd_prog").expect("get default value 'cmd_prog'"),
        defaults.get("cmd_flag").expect("get default value 'cmd_flag'"),
        defaults.get("plc_path_all").expect("get default value 'plc_path_all'").replace("{}", "n")
      )
    ),
    (
      "plus", format!(
        "The '{}' signal can be used before the EXTENSION etc. to avoid both the save and run stages, or before the COMMAND to avoid run only. The '{}' placeholder can be used in a full PATH to denote the default or overridden output directory name.",
        defaults.get("sig_stop").expect("get default value 'sig_stop'"),
        defaults.get("plc_path_dir").expect("get default value 'plc_path_dir'")
      )
    ),
    (
      "pipe", format!(
        "One or more file paths can be piped to aliesce to append the content at each to the source as a script, auto-preceded by a tag line with a base '{}', then exit.",
        defaults.get("sig_stop").expect("get default value 'sig_stop'")
      )
    )
  ];

  ConfigMessages {
    repository: HashMap::from(repository),
    keys_notes: Vec::from(["file", "line", "main", "plus", "pipe"])
  }
}

/* - MAIN */

fn main() {

  /* INITIAL SETUP */

  let defaults = HashMap::from(DEFAULTS);
  let settings = settings_new(&defaults);
  let messages = messages_new(&defaults);

  let config_init = Config {
    defaults,
    settings,
    messages,
    receipts: HashMap::new()
  };

  /* update config for args passed to command */
  let args_on_cli = env::args()
    .skip(1)
    .collect::<Vec<_>>();
  let config_base = Config::receive(config_init, &args_remaining_cli_apply, args_on_cli);

  /* SOURCE APPEND VIA STDIN */

  if_paths_on_stdin_push_then_exit(&config_base);

  /* SOURCE UPDATE VIA ARGS OR PROCESS TO OUTPUT */

  let source = source_get(&config_base);

  /* update config for args passed in source */
  let args_in_src = source.preface
    .split_whitespace()
    .map(|part| part.trim().to_string())
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>();
  let config_full = Config::receive(config_base, &args_remaining_src_apply, args_in_src);

  if_change_in_args_make_then_exit(&source, &config_full);

  /* get outputs and output subset as context */
  let outputs = outputs_get(source, &config_full);
  let context = context_get(&outputs);

  /* print output if text or process if file */
  outputs
    .iter()
    .for_each(|o| o.apply(&context))
}

/* - data structures */

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
    let body = lines
      .collect::<Vec<_>>()
      .join("\n");

    Script { n, line, body }
  }
}

/* - primary functions */

/*   - general */

fn if_paths_on_stdin_push_then_exit(config: &Config) {

  let (tx, rx) = mpsc::channel();

  /* spawn thread for blocking read and send bytes */
  thread::spawn(move || {
    let mut stdin = io::stdin();
    let mut bfr;
    loop {
      bfr = [0; 512];
      match stdin.read(&mut bfr) {
        Ok(0)  => break,
        Ok(_)  => tx.send(bfr).unwrap(),
        Err(e) => {
          format!("Failed (read error: '{e}')");
          process::exit(1);
        }
      }
    }
  });
  thread::sleep(Duration::from_millis(5));

  /* receive bytes and build string */
  let mut recvd = String::new();
  loop {
    thread::sleep(Duration::from_micros(25));
    match rx.try_recv() {
      Ok(b)  => recvd.push_str(&String::from_utf8(b.to_vec()).unwrap()),
      Err(_) => break
    };
  }

  /* process lines in string to paths */
  let paths = recvd
    .trim_end_matches("\0")
    .split_whitespace()
    .map(|s| s.to_string())
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>();

  /* handle script pushes for any paths */
  if !paths.is_empty() {
    for path in paths {
      let strs = Vec::from([
        config.defaults.get("sig_stop").unwrap().to_string(),
        path
      ]);
      script_push(&config, strs);
    }
    process::exit(0);
  };
}

fn if_change_in_args_make_then_exit(source: &Source, config: &Config) {

  let args = match config.receipts.get("edit") {
    Some(ConfigReceiptVal::Strs(s)) => s.to_owned(),
    _                            => Vec::new()
  };

  /* handle source changes for any args */
  if !args.is_empty() {

    let arg_n = args[0].parse::<usize>().expect("parse no. for option 'edit'");
    let arg_line = &args[1];
    let arg_line_tagged = tag_head_add(arg_line, &config);

    /* update tag line and join whole */
    let source_scripts = source.scripts.iter()
      .map(|script| {
        let Script { n, line, body } = script;
        let line_tagged = tag_head_add(line, &config);
        format!("{}\n{body}\n", if arg_n == *n { &arg_line_tagged } else { &line_tagged })
      })
      .collect::<String>();

    let text = format!("{}{source_scripts}", source.preface);

    /* write source to file, with backup to then removal of temporary directory */
    let path_src      = config.get("path_src", "path_src");
    let path_src_inst = Path::new(&path_src);
    let path_src_stem = path_src_inst.file_stem().unwrap().to_str().unwrap();
    let path_src_ext  = path_src_inst.extension().unwrap().to_str().unwrap();

    let secs = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .unwrap()
      .as_secs();

    let path_tmp_dir = config.defaults.get("path_tmp_dir").unwrap();
    let path_tmp = format!("{path_tmp_dir}/{path_src_stem}_{secs}.{path_src_ext}");

    fs::create_dir_all(&path_tmp_dir)
      .unwrap_or_else(|_| panic!("create temporary directory '{path_tmp_dir}' for source backup"));
    fs::copy(&path_src, &path_tmp)
      .unwrap_or_else(|_| panic!("copy source as backup to '{path_tmp}'"));
    fs::write(&path_src, text)
      .unwrap_or_else(|_| panic!("write updated source to '{path_src}'"));
    fs::remove_dir_all(&path_tmp_dir)
      .unwrap_or_else(|_| panic!("remove temporary directory '{path_tmp_dir}'"));

    println!("Updated tag line for script no. {arg_n} to '{arg_line_tagged}'");
    process::exit(0);
  };
}

fn source_get(config: &Config) -> Source {

  let doc_line_file = config.messages.repository.get("file")
    .expect("get message 'file' from configuration");
  let doc_line_line = config.messages.repository.get("line")
    .expect("get message 'line' from configuration");

  /* load source file content as string or exit early */
  let sections = fs::read_to_string(&config.get("path_src", "path_src"))
    .unwrap_or_else(|e| error_handle((
      &format!("Not parsing source file '{}'", config.get("path_src", "path_src")),
      Some("read"),
      Some(e)
    )))
    /* set any init option text with tag head to placeholder */
    .lines()
    .map(|l| if doc_line_file == &l { "plc_doc_line_file" } else { l })
    .map(|l| if doc_line_line == &l { "plc_doc_line_line" } else { l })
    .collect::<Vec<_>>()
    .join("\n")
    /* get args section plus each source string (script with tag line minus tag head) numbered */
    .split(config.defaults.get("tag_head").unwrap())
    .map(|part| part.to_owned())
    .enumerate()
    /* remove any shebang line */
    .map(|(i, part)| if 0 == i && part.len() >= 2 && "#!" == &part[..2] {
        (i, part.splitn(2, '\n').last().unwrap().to_string())
      } else {
        (i, part)
    })
    .collect::<Vec<_>>();

  let preface = sections[0].1
    /* restore any init option text set to placeholder */
    .replace("plc_doc_line_file", doc_line_file)
    .replace("plc_doc_line_line", doc_line_line);
  let scripts = Vec::from(sections.split_at(1).1)
    .iter()
    .map(|section| Script::new(section.0, section.1.to_owned()))
    .collect::<Vec<_>>();

  Source { preface, scripts }
}

fn inputs_parse(script: &Script, config: &Config) -> Output {

  let Script { n, line, body } = script;
  let Config { defaults, receipts, .. } = config;

  /* get label and data from tag line */
  let line_sections = match line.find(defaults.get("tag_tail").unwrap()) {
    Some(i) => line.split_at(i + 1),
    None    => ("", line.as_str())
  };
  let line_label = line_sections.0
    .split(defaults.get("tag_tail").unwrap())
    .nth(0)
    .unwrap(); /* untrimmed */
  let line_data  = line_sections.1.trim();

  /* handle option - list - print only */
  if receipts.contains_key("list") {
    let join = if !line_label.is_empty() { [line_label, ":"].concat() } else { String::from("") };
    let text = format!("{n}:{join} {line_data}");
    return Output::Text(OutputText::Stdout(text));
  };

  /* get items from tag line data */
  let data = line_data.split(' ')
    .map(|item| item.to_string())
    .filter(|item| !item.is_empty()) /* remove whitespace */
    .collect::<Vec<_>>();

  /* handle data absent or bypass */
  if data.is_empty() {
    let text = format!("No tag data found for script no. {n}");
    return Output::Text(OutputText::Stderr(text));
  }
  if data.get(0).unwrap() == defaults.get("sig_stop").unwrap() {
    let text = format!("Bypassing script no. {n} ({} applied)", defaults.get("sig_stop").unwrap());
    return Output::Text(OutputText::Stderr(text));
  }

  Output::File(OutputFile::new(data, body.to_owned(), n.to_owned(), config))
}

fn outputs_get(source: Source, config: &Config) -> Vec<Output> {
  source.scripts
    .iter()
    /* handle option - only - allow subset */
    .filter(|script| !config.receipts.contains_key("only") || match config.receipts.get("only").unwrap() {
      ConfigReceiptVal::Ints(ns) => ns.contains(&script.n),
      _                       => false
    })
    /* parse input set to output instance */
    .map(|script| inputs_parse(script, &config))
    .collect::<Vec<_>>()
}

fn context_get(outputs: &Vec<Output>) -> HashMap<usize, String> {
  outputs
    .iter()
    /* get each output path with script no. */
    .fold(HashMap::new(), |mut acc: HashMap<usize, String>, output| {
      if let Output::File(file) = output { acc.insert(file.n, file.path.get()); }
      acc
    })
}

/*   - argument applicators */

fn setting_dest_apply(_: &Config, strs: Vec<String>) -> ConfigReceiptVal {
  ConfigReceiptVal::Strs(strs)
}

fn setting_edit_apply(_: &Config, strs: Vec<String>) -> ConfigReceiptVal {
  ConfigReceiptVal::Strs(strs)
}

fn setting_list_apply(_0: &Config, _1: Vec<String>) -> ConfigReceiptVal {
  ConfigReceiptVal::Bool
}

fn setting_only_apply(_: &Config, strs: Vec<String>) -> ConfigReceiptVal {
  let val_ints = strs[0]
    .trim()
    .split(',')
    .flat_map(|val_str| {
      let vals = val_str
        .trim()
        .split('-')
        .map(|item| item.parse::<usize>().expect("parse subset for option 'only'"))
        .collect::<Vec<_>>();
      if vals.len() > 1 {
        (vals[0]..(vals[1] + 1))
          .collect::<Vec<_>>()
      } else {
         vals
      }
    })
    .collect::<Vec<_>>();
  ConfigReceiptVal::Ints(val_ints)
}

fn setting_push_apply(config: &Config, strs: Vec<String>) -> ConfigReceiptVal {
  script_push(config, strs);
  process::exit(0);
}

fn setting_init_apply(config: &Config, _: Vec<String>) -> ConfigReceiptVal {

  let src = &config.get("path_src", "path_src");
  let summary_failure_write = format!("Not creating template source file at '{src}'");

  /* exit early if source file exists */
  if fs::metadata(src).is_ok() {
    error_handle((
      &format!("{summary_failure_write} (path exists)"),
      None,
      None
    ))
  };

  let summary_expect_get = "get message from configuration for template source file";
  let content = format!("\
      <any arguments to aliesce (run 'aliesce --help' for options)>\n\n\
      Notes on source file format:\n\n\
      {}\n\n{}\n\n{}\n\n\
      Appending scripts via stdin:\n\n\
      {}\n\n\
      Tag line and script section:\n\n\
      {}\n\n<script>\n\
    ",
    config.messages.repository.get("file").expect(&format!("{summary_expect_get} ('file')")),
    config.messages.repository.get("main").expect(&format!("{summary_expect_get} ('main')")),
    config.messages.repository.get("plus").expect(&format!("{summary_expect_get} ('plus')")),
    config.messages.repository.get("pipe").expect(&format!("{summary_expect_get} ('pipe')")),
    config.messages.repository.get("line").expect(&format!("{summary_expect_get} ('line')"))
  );

  fs::write(src, content)
    .unwrap_or_else(|e| error_handle((
      &summary_failure_write,
      Some("write"),
      Some(e)
    )));

  println!("Created template source file at '{src}'");
  process::exit(0);
}

fn args_remaining_cli_apply(args_remaining: Vec<String>) -> ConfigReceipts {
  /* set final source filename (incl. output stem) per positional arg */
  let mut receipts = ConfigReceipts::new();
  if !args_remaining.is_empty() {
    let arg = args_remaining.get(0).unwrap().clone();
    let val = ConfigReceiptVal::Strs(Vec::from([arg]));
    receipts.insert(String::from("path_src"), val);
  }
  receipts
}

fn args_remaining_src_apply(_: Vec<String>) -> ConfigReceipts {
  ConfigReceipts::new()
}

/* - utility functions */

fn tag_head_add(line: &str, config: &Config) -> String {
  let tag_head = config.defaults.get("tag_head").unwrap();
  if line.len() >= 3 && line[..3] == **tag_head { line.to_string() } else { format!("{tag_head} {}", line.trim()) }
}

fn script_push(config: &Config, strs: Vec<String>) {

  let script_filename = &strs[1];

  /* handle read */

  let script = fs::read_to_string(script_filename)
    .unwrap_or_else(|e| error_handle((
      &format!("Not parsing script file '{script_filename}'"),
      Some("read"),
      Some(e)
    )));
  let tag_line = tag_head_add(&strs[0], &config);
  let script_plus_tag_line = format!("\n{tag_line}\n\n{script}");

  /* handle write */

  let summary_base = format!(
    "tag line '{tag_line}' and content of script file '{script_filename}' to source file '{}'",
    config.get("path_src", "path_src")
  );
  let summary_failure = format!("Not appending {summary_base}");
  let summary_success = format!("Appended {summary_base}");

  fs::OpenOptions::new()
    .append(true)
    .open(config.get("path_src", "path_src"))
    .unwrap_or_else(|e| error_handle((
      &summary_failure,
      Some("open"),
      Some(e)
    )))
    .write_all(&script_plus_tag_line.into_bytes())
    .unwrap_or_else(|e| error_handle((
      &summary_failure,
      Some("write"),
      Some(e)
    )));

  println!("{summary_success}");
}

fn error_handle(strs: (&String, Option<&str>, Option<io::Error>)) -> ! {
  match strs {
    (s, Some(a), Some(e)) => eprintln!("{s} ({a} error: '{e}')"),
    (s, None,    None   ) => eprintln!("{s}"),
    _                     => eprintln!("Failed (unknown error)")
  }
  process::exit(1);
}

/* OUTPUT */

mod output {

  /* - imports */

  use std::fs;
  use std::process;
  use std::collections::HashMap;

  use crate::config::{Config};

  /* - data structures */

  #[derive(Debug, PartialEq)]
  pub enum Output {
    Text(OutputText),
    File(OutputFile)
  }

  impl Output {

    pub fn apply(&self, context: &HashMap<usize, String>) {
      match self {
        Output::Text(e) => {
          match e {
            OutputText::Stdout(s) => {  println!("{s}"); },
            OutputText::Stderr(s) => { eprintln!("{s}"); }
          }
        },
        Output::File(s) => {
          s.save();
          s.exec(&context);
        }
      };
    }
  }

  #[derive(Debug, PartialEq)]
  pub enum OutputText {
    Stdout(String),
    Stderr(String)
  }

  #[derive(Debug, PartialEq)]
  pub struct OutputFile {
    pub data: Vec<String>,
    pub code: String,
    pub path: OutputFilePath,
    pub init: OutputFileInit,
    pub n:    usize
  }

  impl OutputFile {

    pub fn new(data: Vec<String>, code: String, n: usize, config: &Config) -> OutputFile {

      let Config { defaults, receipts: _, .. } = config;

      /* set output path parts */

      /* get output path parts - break first data item on '/' */
      let mut parts_path = data.get(0).unwrap()
        .split('/')
        .collect::<Vec<_>>();
      let path_dir = config.get("dest", "path_dir");

      /* handle output directory identified by directory placeholder */
      if defaults.get("plc_path_dir").unwrap() == &parts_path[0] { parts_path[0] = path_dir.as_str() };

      /* get output filename parts - separate last output path part and break on '.' */
      let parts_filename = parts_path
        .split_off(parts_path.len() - 1)
        .last()
        .unwrap()
        .split('.')
        .collect::<Vec<_>>();
      let p_f_len = parts_filename.len();

      /* set as dir either remaining output path parts recombined or directory name,
             as stem either all but last output filename part or src stem, and
             as ext last output filename part */
      let dir = if !parts_path.is_empty() { parts_path.join("/") } else { path_dir.to_string() };
      let stem = if p_f_len > 1 {
        parts_filename[..(p_f_len - 1)]
          .join(".")
      } else {
        config.get("path_src", "path_src")
          .split('.')
          .nth(0)
          .unwrap()
          .to_string()
      };
      let ext = parts_filename
        .iter()
        .last()
        .unwrap()
        .to_string();

      let path = OutputFilePath{ dir, stem, ext };

      /* set output init parts */

      /* handle file run precluded */
      if data.len() == 1 {
        let init = OutputFileInit::Text(
          OutputText::Stderr(
            format!("Not running file no. {n} (no values)")
          )
        );
        return OutputFile { data, code, path, init, n };
      }
      if data.get(1).unwrap() == defaults.get("sig_stop").unwrap() {
        let init = OutputFileInit::Text(
          OutputText::Stderr(
            format!("Not running file no. {n} ({} applied)", defaults.get("sig_stop").unwrap())
          )
        );
        return OutputFile { data, code, path, init, n };
      }

      /* set as plcs any uses of output path placeholder and note presence as indicator of composite command */
      let mut parts_placeholder = defaults.get("plc_path_all").unwrap().split("{}");
      let plc_head = parts_placeholder.next().unwrap();
      let plc_tail = parts_placeholder.next().unwrap();
      let plc_full = Vec::from([plc_head, plc_tail]).join("");

      let plcs = data
        .iter()
        .skip(1)
        .map(|item| {
          /* handle request for current script output path */
          if item.contains(&plc_full) { return (0, plc_full.to_owned()) };
          let head_i = if let Some(i) = item.find(plc_head) { i as i8 } else { -1 };
          let tail_i = if let Some(i) = item.find(plc_tail) { i as i8 } else { -1 };
          /* handle request for another script output path */
          if -1 != head_i && -1 != tail_i && head_i < tail_i {
             let s = item
               .chars()
               .skip(head_i as usize)
               .take((tail_i - head_i + 1) as usize)
               .collect::<String>();
             let i = s
               .chars()
               .skip(plc_head.len())
               .take(s.len() - plc_full.len())
               .collect::<String>()
               .parse::<i8>()
               .unwrap();
             return (i, s)
          };
          /* handle no request - value to be filtered out */
          (-1, String::new())
        })
        .filter(|item| -1 != item.0)
        .collect::<Vec<_>>();

      let has_placeholder = !plcs.is_empty();

      /* set as prog either tag line second item or default, and
             as args either Vec containing remaining items plus combined path or default flag plus remaining items joined */
      let prog = String::from(if has_placeholder { *defaults.get("cmd_prog").unwrap() } else { data.get(1).unwrap() });
      let args = if has_placeholder {
        Vec::from([
          defaults.get("cmd_flag").unwrap().to_string(),
          data
            .iter()
            .skip(1)
            .map(|item| item.to_owned())
            .collect::<Vec<_>>()
            .join(" ")
        ])
      } else {
        [
          data
            .iter()
            .skip(2)
            .map(|arg| arg.to_owned())
            .collect::<Vec<_>>(),
          Vec::from([path.get()])
        ]
          .concat()
      };

      let init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

      OutputFile { data, code, path, init, n }
    }

    fn save(&self) {

      let OutputFile { data: _, code, path, init: _, n: _ } = self;
      let dir = &path.dir;
      let path = path.get();

      /* add directory if none */
      fs::create_dir_all(&dir).unwrap_or_else(|_| panic!("create directory '{dir}'"));
      /* write script to file */
      fs::write(&path, code).unwrap_or_else(|_| panic!("write script to '{path}'"));
    }

    fn exec(&self, context: &HashMap<usize, String>) {

      let OutputFile { data: _, code: _, path: _, init, n } = self;

      match init {

        /* print reason file run precluded */
        OutputFileInit::Text(e) => {
          match e {
            OutputText::Stdout(s) => {  println!("{s}"); },
            OutputText::Stderr(s) => { eprintln!("{s}"); }
          }
        },
        /* run script from file */
        OutputFileInit::Code(c) => {
          let OutputFileInitCode { prog, args, plcs } = c;

          let args_full = if plcs.is_empty() {
            args.to_owned()
          } else {
            let mut cmd = if 0 == plcs.len() { String::new() } else { args[1].to_owned() };
            plcs
              .iter()
              .for_each(|plc| {
                let path = if 0 == plc.0 { context.get(n).unwrap() } else { context.get(&(plc.0 as usize)).unwrap() };
                cmd = cmd.replace(plc.1.as_str(), path.as_str()).to_owned();
              });
            Vec::from([args[0].to_owned(), cmd])
          };

          process::Command::new(&prog)
            .args(args_full)
            .spawn()
            .unwrap_or_else(|_| panic!("run file with '{prog}'"))
            .wait_with_output()
            .unwrap_or_else(|_| panic!("await output from '{prog}'"));
        }
      }
    }
  }

  #[derive(Debug, PartialEq)]
  pub struct OutputFilePath {
    pub dir:  String,
    pub stem: String,
    pub ext:  String
  }

  impl OutputFilePath {
    pub fn get(&self) -> String {
      format!("{}/{}.{}", &self.dir, &self.stem, &self.ext)
    }
  }

  #[derive(Debug, PartialEq)]
  pub enum OutputFileInit {
    Text(OutputText),
    Code(OutputFileInitCode)
  }

  #[derive(Debug, PartialEq)]
  pub struct OutputFileInitCode {
    pub prog: String,
    pub args: Vec<String>,
    pub plcs: Vec<(i8, String)>
  }
}

/* CONFIG, incl. argument_handling */

mod config {

  /* - imports */

  use std::process;
  use std::collections::HashMap;

  /* - data structures */

  pub struct Config<'a> {
    pub defaults: ConfigDefaults<'a>,
    pub settings: ConfigSettings,
    pub receipts: ConfigReceipts,
    pub messages: ConfigMessages<'a>
  }

  impl Config<'_> {

    pub fn receive(mut config: Config<'static>, handle_remaining: &ArgHandler, args: Vec<String>) -> Config<'static> {

      let args_count: usize = args.len();

      /* for each flag in args, queue setting call with any values and tally */
      let mut settings_queued = Vec::new();
      let mut settings_count = 0;
      if args_count > 0 {
        for setting in &config.settings {
          for j in 0..args_count {
            if ["--", &setting.word].concat() == args[j] || ["-", &setting.char].concat() == args[j] {
              let strs_len = setting.strs.len();
              let strs = args[(j + 1)..(j + strs_len + 1)].to_vec();
              settings_queued.push((&setting.word, &setting.call, strs));
              settings_count = settings_count + 1 + strs_len;
            };
          };
        };
      };
      /* handle any remaining arguments */
      let args_remaining = args[(settings_count)..].to_vec();
      let receipts_args_remaining = handle_remaining(args_remaining);
      config.receipts.extend(receipts_args_remaining);

      /* make any queued setting calls */
      if !settings_queued.is_empty() {
        for opt_queued in &settings_queued {
          let (word, call, strs) = &opt_queued;
          let value = call(&config, strs.to_vec());
          config.receipts.insert(String::from(*word), value);
        }
      }
      config
    }

    pub fn get(&self, key_receipt: &str, key_default: &str) -> String {
      if self.receipts.contains_key(key_receipt) {
        if let ConfigReceiptVal::Strs(val_strs) = self.receipts.get(key_receipt).unwrap() {
          return val_strs
            .get(0)
            .expect(&format!("get string for receipt value '{key_receipt}' from configuration"))
            .to_string();
        }
      }
      String::from(
        *self.defaults
          .get(key_default)
          .expect(&format!("get default value '{key_default}' from configuration"))
      )
    }
  }

  pub type ConfigDefaults<'a> = HashMap<&'a str, &'a str>;
  pub type ConfigSettings = Vec<ConfigSetting>;
  pub type ConfigReceipts = HashMap<String, ConfigReceiptVal>;

  #[derive(PartialEq, Eq)]
  pub enum ConfigReceiptVal {
    Bool,
    Ints(Vec<usize>),
    Strs(Vec<String>)
  }

  pub struct ConfigMessages<'a> {
    pub repository: HashMap<&'a str, String>,
    pub keys_notes: Vec<&'a str>
  }

  impl ConfigMessages<'_> {

    pub fn compose_notes(&self) -> Vec<String> {
      self.keys_notes
        .iter()
        .map(|k|
          self.repository
            .get(k)
            .expect(&format!("get message '{k}' from configuration for notes"))
            .to_string()
        )
        .collect()
    }
  }

  type ConfigSettingCall = dyn Fn(&Config, Vec<String>) -> ConfigReceiptVal;

  pub struct ConfigSetting {
    pub word: String,
    pub char: String,
    pub strs: Vec<String>,
    pub desc: String,
        call: Box<ConfigSettingCall>
  }

  impl ConfigSetting {
    pub fn new(word: &str, char: &str, val_strs: &[&str], desc: &str, call: &'static ConfigSettingCall) -> ConfigSetting {
      let strs = if !val_strs.is_empty() {
        val_strs
          .iter()
          .map(|&s| String::from(s))
          .collect::<Vec<_>>()
      } else {
        Vec::new()
      };
      ConfigSetting {
        word: String::from(word),
        char: String::from(char),
        strs,
        desc: String::from(desc),
        call: Box::new(call)
      }
    }
    pub fn new_version() -> ConfigSetting {
      ConfigSetting::new("version", "v", &[], "show name and version number then exit", &setting_version_apply)
    }
    pub fn new_help() -> ConfigSetting {
      ConfigSetting::new("help", "h", &[], "show usage, flags available and notes then exit", &setting_help_apply)
    }
  }

  type ArgHandler = dyn Fn(Vec<String>) -> ConfigReceipts;

  /* - argument applicator ('help') */

  fn setting_version_apply(_0: &Config, _1: Vec<String>) -> ConfigReceiptVal {
    println!("{}", name_and_version_get());
    process::exit(0);
  }

  fn setting_help_apply(config: &Config, _: Vec<String>) -> ConfigReceiptVal {

    let line_length_max = 80;

    /* set value substrings and max length */
    let strs_strs = config.settings
      .iter()
      .map(|o| o.strs.join(" "))
      .collect::<Vec<_>>();
    let strs_strs_max = strs_strs.iter()
      .fold(0, |acc, s| if s.len() > acc { s.len() } else { acc });
    let flag_strs = config.settings
      .iter()
      .map(|o| format!("-{}, --{}", o.char, o.word))
      .collect::<Vec<_>>();
    let flag_strs_max = flag_strs
      .iter()
      .fold(0, |acc, s| if s.len() > acc { s.len() } else { acc });

    /* generate title line */
    let title_line = format!("{}", line_center_with_fill(&name_and_version_get(), line_length_max, "-"));

    /* generate usage text */
    let usage_opts_part = config.settings
      .iter()
      .filter(|o| o.word != "version" && o.word != "help") /* avoid duplication */
      .enumerate() /* yield also index (i) */
      .map(|(i, o)| format!(
        "[--{}/-{}{}]",
        o.word,
        o.char,
        if strs_strs[i].is_empty() { String::from("") } else { [" ", &strs_strs[i]].concat() })
      )
      .collect::<Vec<_>>()
      .join(" ");
    let usage_opts_head = line_break_and_indent(&format!("{usage_opts_part} [SOURCE]"), 15, line_length_max, false);
    let usage_opts_tail = line_break_and_indent(&format!("/ --version/-v / --help/-h"), 15, line_length_max, true);
    let usage_text = format!("Usage: aliesce {usage_opts_head}\n{usage_opts_tail}");

    /* generate flags text */
    let flags_list = config.settings
      .iter()
      .enumerate() /* yield also index (i) */
      .map(|(i, o)| {
        let desc = line_break_and_indent(&o.desc, flag_strs_max + strs_strs_max + 2, line_length_max, false);
        format!(" {}  {:w$}  {desc}", flag_strs[i], strs_strs[i], w = flag_strs_max - o.word.len())
      })
      .collect::<Vec<_>>()
      .join("\n");
    let flags_text = format!("Flags:\n{flags_list}");

    /* generate notes text */
    let notes_body = config.messages.compose_notes()
      .iter()
      .map(|l| line_break_and_indent(&l, 1, line_length_max, true))
      .collect::<Vec<_>>()
      .join("\n\n");
    let notes_text = format!("Notes:\n{notes_body}");

    println!("{title_line}\n\n{usage_text}\n{flags_text}\n\n{notes_text}");
    process::exit(0);
  }

  /* - utility functions */

  fn name_and_version_get() -> String {
    format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
  }

  fn line_center_with_fill(line: &str, length: usize, fill: &str) -> String {
    let whitespace_half = String::from(fill).repeat((length - line.len() - 2) / 2);
    let whitespace_last = if 0 == line.len() % 2 { "" } else { fill };
    format!("{whitespace_half} {line} {whitespace_half}{whitespace_last}")
  }

  fn line_break_and_indent(line: &str, indent: usize, length: usize, indent_first: bool ) -> String {

    let whitespace_part = String::from(" ").repeat(indent);
    let whitespace_full = format!("\n{whitespace_part}");
    let text_width = length - indent;

    let body = line
      .split(' ')
      .collect::<Vec<_>>()
      .iter()
      .fold(Vec::new(), |mut acc: Vec<String>, word| {
        if acc.is_empty() { return Vec::from([String::from(*word)]) };
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

    if indent_first { format!("{whitespace_part}{body}") } else { body }
  }
}

/* TEST */

#[cfg(test)]
mod test {

  /* - imports */

  use::std::io::Write;
  use::std::fs;
  use::std::process;
  use::std::collections::HashMap;

  use super::{
    DEFAULTS,
    Script,
    settings_new,
    messages_new,
    inputs_parse
  };
  use crate::output::{
    Output,
    OutputText,
    OutputFile,
    OutputFilePath,
    OutputFileInit,
    OutputFileInitCode
  };
  use crate::config::{
    Config,
    ConfigReceiptVal
  };

  /* - test cases */

  /*   - end-to-end */

  /*     - stdin read, settings */

  const PATH_TMP_DIR_TEST: &str = "./.test_temp";

  fn test_values_script_get(path_dir: &String, n: u8) -> (String, String, String, String, String) {
    let output_filename = format!("test_{n}.sh");
    let string = format!("Running {n}");
    let output = format!("{string}\n");
    (
      format!("{path_dir}/script_{n}.txt"),
      format!(">/{output_filename} sh"),
      format!("echo \"{string}\"\n"),
      output_filename,
      output
    )
  }

  fn test_values_end_to_end_get() -> [String; 23] {

    let path_dir = String::from(PATH_TMP_DIR_TEST);
    let path_dir_scripts = format!("{path_dir}/scripts");
    let path_source      = format!("{path_dir}/source.txt");

    let (
      path_script_1, content_script_line_base_1, content_script_body_1,
      content_script_output_filename_1, content_script_output_1
    ) = test_values_script_get(&path_dir, 1);
    let (
      path_script_2, content_script_line_base_2, content_script_body_2,
      content_script_output_filename_2, content_script_output_2
    ) = test_values_script_get(&path_dir, 2);
    let (
      path_script_3, _,                          content_script_body_3,
       _,                                content_script_output_3
    ) = test_values_script_get(&path_dir, 3);

    let content_source_preface = String::from("Test preface\n");
    let content_source_script_line = format!("{} sh sh\n", DEFAULTS[3].1);
    let content_source_script_body = format!("echo \"Running initial\"\n");

    let content_source_single = format!("{content_source_preface}{content_source_script_line}{content_source_script_body}");

    let content_script_line_label = format!("Test label");

    let content_script_line_tagged          = format!("{} {content_script_line_base_1}", DEFAULTS[3].1);
    let content_script_line_tagged_labelled = format!("{} {content_script_line_label} {} {content_script_line_base_2}", DEFAULTS[3].1, DEFAULTS[4].1);
    let content_script_line_tagged_bypass   = format!("{} {}", DEFAULTS[3].1, DEFAULTS[5].1);

    let content_source_triple = format!("{content_source_preface}{content_script_line_tagged}\n{content_script_body_1}{content_script_line_tagged_labelled}\n{content_script_body_2}{content_script_line_tagged_bypass}\n{content_script_body_3}");

    [
      path_dir, path_dir_scripts, path_source, path_script_1, path_script_2, path_script_3,
      content_script_output_filename_1, content_script_output_filename_2,
      content_source_preface, content_source_script_body, content_source_single, content_source_triple,
      content_script_line_base_1, content_script_line_base_2, content_script_line_tagged, content_script_line_tagged_bypass, content_script_line_label,
      content_script_body_1, content_script_body_2, content_script_body_3,
      content_script_output_1, content_script_output_2, content_script_output_3
    ]
  }

  fn test_tree_create(files: Vec<[&str; 3]>) {
    let path_dir = &test_values_end_to_end_get()[0];
    fs::create_dir_all(&path_dir)
      .unwrap_or_else(|_| panic!("create temporary test directory '{path_dir}'"));
    for file in files {
      let [path_file, content_file, description] = file;
      fs::write(&path_file, &content_file).unwrap_or_else(|_| panic!("write {description} to '{path_file}'"));
    }
  }

  fn test_tree_remove() {
    let path_dir = &test_values_end_to_end_get()[0];
    fs::remove_dir_all(&path_dir)
      .unwrap_or_else(|_| panic!("remove temporary test directory '{path_dir}'"));
  }

  /*     - stdin read */

  fn test_stdin_read_run(input_delimiter: &str) -> () {

    let [
      _, _, path_source, path_script_1, path_script_2, path_script_3,
      _, _,
      _, _, content_source_single, _,
      _, _, _, content_script_line_tagged_bypass, _,
      content_script_body_1, content_script_body_2, content_script_body_3,
      _, _, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source,   &content_source_single, "test source"       ],
      [&path_script_1, &content_script_body_1, "test script 1 body"],
      [&path_script_2, &content_script_body_2, "test script 2 body"],
      [&path_script_3, &content_script_body_3, "test script 3 body"]
    ]));

    /* acquisitions */

    let mut proc = process::Command::new("cargo")
      .args(Vec::from(["run", "--", &path_source]))
      .stdin(process::Stdio::piped())
      .stdout(process::Stdio::piped())
      .stderr(process::Stdio::piped())
      .spawn()
      .unwrap();

    let input = format!("{path_script_1}{d}{path_script_2}{d}{path_script_3}", d = input_delimiter);

    proc.stdin
      .take()
      .unwrap()
      .write_all(input.as_bytes())
      .unwrap();
    let output_raw = proc
      .wait_with_output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);
    let source = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));
    let source_line_1 = source.lines().nth( 4).unwrap();
    let source_line_2 = source.lines().nth( 8).unwrap();
    let source_line_3 = source.lines().nth(12).unwrap();

    test_tree_remove();

    /* assertions */

    assert!(output.contains(&content_script_line_tagged_bypass));
    assert!(output.contains(&path_script_1));
    assert!(output.contains(&path_script_2));
    assert!(output.contains(&path_script_3));

    assert!(source.contains(&content_source_single));
    assert_eq!(content_script_line_tagged_bypass, source_line_1);
    assert_eq!(content_script_line_tagged_bypass, source_line_2);
    assert_eq!(content_script_line_tagged_bypass, source_line_3);
    assert!(source.contains(&content_script_body_1));
    assert!(source.contains(&content_script_body_2));
    assert!(source.contains(&content_script_body_3));
  }

  #[test]
  fn stdin_read() {

    let input_delimiter_1 = " ";
    let input_delimiter_2 = "\n";

    test_stdin_read_run(input_delimiter_1);
    test_stdin_read_run(input_delimiter_2);
  }

  /*     - settings */

  #[test]
  fn setting_dest() {

    let [
      _, path_dir_scripts, path_source, _, _, _,
      content_script_output_filename_1, content_script_output_filename_2,
      _, _, _, content_source_triple,
      _, _, _, _, _,
      content_script_body_1, content_script_body_2, _,
      content_script_output_1, content_script_output_2, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_triple, "test source"]
    ]));

    /* acquisitions */

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-d", &path_dir_scripts, &path_source]))
      .output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);

    let scripts = fs::read_dir(&path_dir_scripts).unwrap()
      .map(|e| e.unwrap().path().display().to_string())
      .collect::<Vec<_>>();
    let scripts_path_1 = format!("{path_dir_scripts}/{content_script_output_filename_1}");
    let scripts_path_2 = format!("{path_dir_scripts}/{content_script_output_filename_2}");
    let scripts_body_1 = fs::read_to_string(&scripts_path_1).unwrap();
    let scripts_body_2 = fs::read_to_string(&scripts_path_2).unwrap();

    test_tree_remove();

    /* assertions */

    assert_eq!(output.to_string(), format!("{content_script_output_1}{content_script_output_2}"));

    assert_eq!(scripts.len(), 2);
    assert!(scripts.contains(&scripts_path_1));
    assert!(content_script_body_1.contains(&scripts_body_1));
    assert!(scripts.contains(&scripts_path_2));
    assert!(content_script_body_2.contains(&scripts_body_2));
  }

  #[test]
  fn setting_only_incl_dest() {

    let [
      _, path_dir_scripts, path_source, _, _, _,
      content_script_output_filename_1, content_script_output_filename_2,
      _, _, _, content_source_triple,
      _, _, _, _, _,
      content_script_body_1, content_script_body_2, _,
      content_script_output_1, content_script_output_2, _
    ] = test_values_end_to_end_get();

    /* setup - one - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_triple, "test source"]
    ]));

    /* acquisitions - one */

    let output_one_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-d", &path_dir_scripts, "-o", "1", &path_source]))
      .output()
      .unwrap();

    let output_one = String::from_utf8_lossy(&output_one_raw.stdout);

    let scripts_one = fs::read_dir(&path_dir_scripts).unwrap()
      .map(|e| e.unwrap().path().display().to_string())
      .collect::<Vec<_>>();
    let scripts_one_path_1 = format!("{path_dir_scripts}/{content_script_output_filename_1}");
    let scripts_one_body_1 = fs::read_to_string(&scripts_one_path_1).unwrap();

    test_tree_remove();

    /* setup - two - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_triple, "test source"]
    ]));

    /* acquisitions - two */

    let output_two_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-d", &path_dir_scripts, "-o", "2-3", &path_source]))
      .output()
      .unwrap();

    let output_two = String::from_utf8_lossy(&output_two_raw.stdout);

    let scripts_two = fs::read_dir(&path_dir_scripts).unwrap()
      .map(|e| e.unwrap().path().display().to_string())
      .collect::<Vec<_>>();
    let scripts_two_path_2 = format!("{path_dir_scripts}/{content_script_output_filename_2}");
    let scripts_two_body_2 = fs::read_to_string(&scripts_two_path_2).unwrap();

    test_tree_remove();

    /* setup - two - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_triple, "test source"]
    ]));

    /* acquisitions - all */

    let output_all_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-d", &path_dir_scripts, "-o", "1,2-3", &path_source]))
      .output()
      .unwrap();

    let output_all = String::from_utf8_lossy(&output_all_raw.stdout);

    let scripts_all = fs::read_dir(&path_dir_scripts).unwrap()
      .map(|e| e.unwrap().path().display().to_string())
      .collect::<Vec<_>>();
    let scripts_all_path_1 = format!("{path_dir_scripts}/{content_script_output_filename_1}");
    let scripts_all_path_2 = format!("{path_dir_scripts}/{content_script_output_filename_2}");
    let scripts_all_body_1 = fs::read_to_string(&scripts_all_path_1).unwrap();
    let scripts_all_body_2 = fs::read_to_string(&scripts_all_path_2).unwrap();

    test_tree_remove();

    /* assertions - one */

    assert_eq!(output_one.to_string(), format!("{content_script_output_1}"));

    assert_eq!(scripts_one.len(), 1);
    assert!(scripts_one.contains(&scripts_one_path_1));
    assert!(content_script_body_1.contains(&scripts_one_body_1));

    /* assertions - two */

    assert_eq!(output_two.to_string(), format!("{content_script_output_2}"));

    assert_eq!(scripts_two.len(), 1);
    assert!(scripts_two.contains(&scripts_two_path_2));
    assert!(content_script_body_2.contains(&scripts_two_body_2));

    /* assertions - all */

    assert_eq!(output_all.to_string(), format!("{content_script_output_1}{content_script_output_2}"));

    assert_eq!(scripts_all.len(), 2);
    assert!(scripts_all.contains(&scripts_all_path_1));
    assert!(content_script_body_1.contains(&scripts_all_body_1));
    assert!(scripts_all.contains(&scripts_all_path_2));
    assert!(content_script_body_2.contains(&scripts_all_body_2));
  }

  #[test]
  fn setting_list() {

    let [
      _, _, path_source, _, _, _,
      _, _,
      _, _, _, content_source_triple,
      content_script_line_base_1, content_script_line_base_2, _, _, content_script_line_label,
      _, _, _,
      _, _, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_triple, "test source"]
    ]));

    /* acquisitions */

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-l", &path_source]))
      .output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);
    let output_lines = output
      .lines()
      .collect::<Vec<_>>();

    test_tree_remove();

    /* assertions */

    assert!(output_lines[0].contains("1"));
    assert!(output_lines[0].contains(&content_script_line_base_1));

    assert!(output_lines[1].contains("2"));
    assert!(output_lines[1].contains(&content_script_line_label));
    assert!(output_lines[1].contains(&content_script_line_base_2));

    assert!(output_lines[2].contains("3"));
    assert!(output_lines[2].contains(DEFAULTS[5].1));
  }

  #[test]
  fn setting_init() {

    let [
      _, _, path_source, _, _, _,
      _, _,
      _, _, _, _,
      _, _, _, _, _,
      _, _, _,
      _, _, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::new());

    /* acquisitions */

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-i", &path_source]))
      .output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);
    let source = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));

    let defaults = HashMap::from(DEFAULTS);
    let settings = settings_new(&defaults);
    let messages = messages_new(&defaults);

    let config_init = Config {
      defaults,
      settings,
      messages,
      receipts: HashMap::new()
    };

    test_tree_remove();

    /* assertions */

    assert!(output.contains(&path_source));
    assert!(source.contains(config_init.messages.repository.get("file").unwrap()));
    assert!(source.contains(config_init.messages.repository.get("line").unwrap()));
    assert!(source.contains(config_init.messages.repository.get("main").unwrap()));
    assert!(source.contains(config_init.messages.repository.get("plus").unwrap()));
    assert!(source.contains(config_init.messages.repository.get("pipe").unwrap()));
  }

  #[test]
  fn setting_push() {

    let [
      _, _, path_source, path_script, _, _,
      _, _,
      _, _, content_source_single, _,
      content_script_line_base_1, _, content_script_line_tagged, _, _,
      content_script_body, _, _,
      _, _, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_single, "test source"     ],
      [&path_script, &content_script_body,   "test script body"]
    ]));

    /* acquisitions */

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-p", &content_script_line_base_1, &path_script, &path_source]))
      .output()
      .unwrap();
    let output = String::from_utf8_lossy(&output_raw.stdout);
    let source = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));
    let source_line = source.lines().nth(4).unwrap();

    let output_tagged_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-p", &content_script_line_tagged, &path_script, &path_source]))
      .output()
      .unwrap();
    let output_tagged = String::from_utf8_lossy(&output_tagged_raw.stdout);
    let source_tagged = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));
    let source_tagged_line = source_tagged.lines().nth(4).unwrap();

    test_tree_remove();

    /* assertions */

    assert!(output.contains(&content_script_line_tagged));
    assert!(output.contains(&path_script));
    assert!(source.contains(&content_source_single));
    assert!(source.contains(&content_script_body));
    assert_eq!(content_script_line_tagged, source_line);

    assert!(output_tagged.contains(&content_script_line_tagged));
    assert!(output_tagged.contains(&path_script));
    assert!(source_tagged.contains(&content_source_single));
    assert!(source_tagged.contains(&content_script_body));
    assert_eq!(content_script_line_tagged, source_tagged_line);
  }

  #[test]
  fn setting_edit() {

    let [
      _, _, path_source, _, _, _,
      _, _,
      content_source_preface, content_source_script_body, content_source_single, _,
      content_script_line_base_1, _, content_script_line_tagged, _, _,
      _, _, _,
      _, _, _
    ] = test_values_end_to_end_get();

    /* setup - add temporary test directory w/ content */
    test_tree_create(Vec::from([
      [&path_source, &content_source_single, "test source"]
    ]));

    let n_script = "1";

    /* acquisitions */

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-e", &n_script, &content_script_line_base_1, &path_source]))
      .output()
      .unwrap();
    let output = String::from_utf8_lossy(&output_raw.stdout);
    let source = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));
    let source_line = source.lines().nth(1).unwrap();

    let output_tagged_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-e", &n_script, &content_script_line_tagged, &path_source]))
      .output()
      .unwrap();
    let output_tagged = String::from_utf8_lossy(&output_tagged_raw.stdout);
    let source_tagged = fs::read_to_string(&path_source)
      .unwrap_or_else(|_| panic!("reading from test source"));
    let source_tagged_line = source_tagged.lines().nth(1).unwrap();

    test_tree_remove();

    /* assertions */

    assert!(output.contains(&n_script));
    assert!(output.contains(&content_script_line_tagged));
    assert!(source.contains(&content_source_preface));
    assert!(source.contains(&content_source_script_body));
    assert_eq!(content_script_line_tagged, source_line);

    assert!(output_tagged.contains(&n_script));
    assert!(output_tagged.contains(&content_script_line_tagged));
    assert!(source_tagged.contains(&content_source_preface));
    assert!(source_tagged.contains(&content_source_script_body));
    assert_eq!(content_script_line_tagged, source_tagged_line);
  }

  #[test]
  fn setting_version() {

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-v"]))
      .output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);
    let output_parts = output
      .split(" v")
      .map(|part| part.trim())
      .collect::<Vec<_>>();

    assert_eq!("aliesce", output_parts[0]);
    assert_eq!(env!("CARGO_PKG_VERSION"), output_parts[1]);
  }

  #[test]
  fn setting_help() {

    let output_raw = process::Command::new("cargo")
      .args(Vec::from(["run", "--", "-h"]))
      .output()
      .unwrap();

    let output = String::from_utf8_lossy(&output_raw.stdout);
    let output_parts_on_usage = output
      .split("Usage:")
      .collect::<Vec<_>>();
    let output_parts_on_flags = output_parts_on_usage[1]
      .split("Flags:")
      .collect::<Vec<_>>();
    let output_parts_on_notes = output_parts_on_flags[1]
      .split("Notes:")
      .collect::<Vec<_>>();

    let defaults = HashMap::from(DEFAULTS);
    let settings = settings_new(&defaults);
    let messages = messages_new(&defaults);

    let config_init = Config {
      defaults,
      settings,
      messages,
      receipts: HashMap::new()
    };
    let messages_notes_line = config_init.messages
      .compose_notes()
      .join(" ");

    /* title section */

    let output_title_part = output_parts_on_usage[0];

    assert!(output_title_part.contains("aliesce"));
    assert!(output_title_part.contains(env!("CARGO_PKG_VERSION")));

    /* usage section */

    let output_usage_line = output_parts_on_flags[0]
      .replace("\n", " ");

    for setting in &config_init.settings {
      let arg_set = format!(
        "--{}/-{} {}",
        setting.word,
        setting.char,
        setting.strs.join(" ")
      );
      assert!(output_usage_line.contains(&arg_set.trim()));
    }

    /* flags section */

    let output_flags_line_condensed = output_parts_on_notes[0]
      .replace("\n", " ")
      .chars()
      .filter(|c| ' ' != *c)
      .collect::<String>();

    for setting in &config_init.settings {
      let flag_line_condensed = format!(
        "-{},--{}{}{}",
        setting.char,
        setting.word,
        setting.strs.join(""),
        setting.desc.replace(" ", "")
      );
      assert!(output_flags_line_condensed.contains(&flag_line_condensed));
    }

    /* notes section */

    let output_notes_line = output_parts_on_notes[1]
      .replace("\n", "")
      .trim()
      .to_string();

    assert_eq!(messages_notes_line, output_notes_line);
  }

  /*   - unit */

  /*     - function: inputs_parse */

  fn test_values_inputs_parse_get() -> (Config<'static>, String, usize, String, OutputFilePath, OutputFileInit) {

    let defaults = HashMap::from(DEFAULTS);
    let settings = settings_new(&defaults);
    let messages = messages_new(&defaults);

    let config_default = Config {
      defaults,
      settings,
      messages,
      receipts: HashMap::new()
    };

    /* base test script values */

    let output_path = OutputFilePath {
      dir:  String::from(*config_default.defaults.get("path_dir").unwrap()),
      stem: String::from( config_default.defaults.get("path_src").unwrap().split(".").nth(0).unwrap()),
      ext:  String::from("ext")
    };

    let body = String::from("//code");

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

    let (config_default, body, n, code, path, init) = test_values_inputs_parse_get();

    let line = String::from(" ext program --flag value\n");
    let data = Vec::from([
      String::from("ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_label_and_data_full_some_output_file() {

    let (config_default, body, n, code, path, init) = test_values_inputs_parse_get();

    let line = String::from(" label # ext program --flag value\n");
    let data = Vec::from([
      String::from("ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_dest_option_some_output_file() {

    let (mut config_default, body, n, code, _, mut init) = test_values_inputs_parse_get();

    let line = String::from(" ext program --flag value\n");
    let data = Vec::from([
      String::from("ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let dir  = String::from("dest");
    let stem = String::from(config_default.defaults.get("path_src").unwrap().split(".").nth(0).unwrap());
    let ext  = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };
    config_default.receipts.insert(String::from("dest"), ConfigReceiptVal::Strs(Vec::from([String::from("dest")])));

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_list_option_some_output_text() {

    let (mut config_default, body, n, _, _, _) = test_values_inputs_parse_get();

    let line = String::from(" ext program --flag value\n");

    config_default.receipts.insert(String::from("list"), ConfigReceiptVal::Bool);

    let expected = Output::Text(OutputText::Stdout(String::from("1: ext program --flag value")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_singlepart_output_stem_some_output_file() {

    let (config_default, body, n, code, _, mut init) = test_values_inputs_parse_get();

    let line = String::from(" script.ext program --flag value\n");
    let data = Vec::from([
      String::from("script.ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let dir  = String::from(*config_default.defaults.get("path_dir").unwrap());
    let stem = String::from("script");
    let ext  = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_multipart_output_stem_some_output_file() {

    let (config_default, body, n, code, _, mut init) = test_values_inputs_parse_get();

    let line = String::from(" script.suffix1.suffix2.ext program --flag value\n");
    let data = Vec::from([
      String::from("script.suffix1.suffix2.ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let dir  = String::from(*config_default.defaults.get("path_dir").unwrap());
    let stem = String::from("script.suffix1.suffix2");
    let ext  = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_dir_some_output_file() {

    let (config_default, body, n, code, _, mut init) = test_values_inputs_parse_get();

    let line = String::from(" dir/script.ext program --flag value\n");
    let data = Vec::from([
      String::from("dir/script.ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let dir  = String::from("dir");
    let stem = String::from("script");
    let ext  = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_dir_placeholder_some_output_file() {

    let (config_default, body, n, code, _, mut init) = test_values_inputs_parse_get();

    let line = String::from(" >/script.ext program --flag value\n");
    let data = Vec::from([
      String::from(">/script.ext"),
      String::from("program"),
      String::from("--flag"),
      String::from("value")
    ]);

    let dir  = String::from("scripts");
    let stem = String::from("script");
    let ext  = String::from("ext");
    let path = OutputFilePath { dir, stem, ext };

    match init { OutputFileInit::Code(ref mut c) => { c.args[2] = path.get() }, _ => () };

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_incl_output_path_all_placeholder_some_output() {

    let (config_default, body, n, code, path, _) = test_values_inputs_parse_get();

    let line = String::from(" ext program_1 --flag value >< | program_2\n");
    let data = Vec::from([
      String::from("ext"),
      String::from("program_1"),
      String::from("--flag"),
      String::from("value"),
      String::from("><"),
      String::from("|"),
      String::from("program_2")
    ]);

    let prog = String::from(*config_default.defaults.get("cmd_prog").unwrap());
    let args = Vec::from([
      String::from(*config_default.defaults.get("cmd_flag").unwrap()),
      String::from("program_1 --flag value >< | program_2")
    ]);
    let plcs = Vec::from([(0, String::from("><"))]);
    let init = OutputFileInit::Code(OutputFileInitCode { prog, args, plcs });

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_minus_cmd_some_output_file_indicating() {

    let (config_default, body, n, code, path, _) = test_values_inputs_parse_get();

    let line = String::from(" ext\n");
    let data = Vec::from([String::from("ext")]);

    let init = OutputFileInit::Text(OutputText::Stderr(String::from("Not running file no. 1 (no values)")));

    let expected = Output::File(OutputFile { data, code, path, init, n });
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_full_with_bypass_some_output_text() {

    let (config_default, body, n, _, _, _) = test_values_inputs_parse_get();

    let line = String::from(" ! ext program --flag value\n");

    let expected = Output::Text(OutputText::Stderr(String::from("Bypassing script no. 1 (! applied)")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn inputs_parse_returns_for_tag_data_absent_some_output_text() {

    let (config_default, body, n, _, _, _) = test_values_inputs_parse_get();

    let line = String::from("\n");

    let expected = Output::Text(OutputText::Stderr(String::from("No tag data found for script no. 1")));
    let obtained = inputs_parse(&Script { n, line, body }, &config_default);

    assert_eq!(expected, obtained);
  }
}
