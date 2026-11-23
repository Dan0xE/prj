use crate::config::Config;
use crate::discovery::cache::{Cache, CacheClient};
use crate::discovery::git::fetch_vcs_info;
use crate::discovery::traverse::Traverser;
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use git2::{Repository, StatusOptions};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};
use std::cmp::Reverse;
use std::io::{stderr, BufWriter, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use term_size::dimensions_stderr;
use thiserror::Error;

#[derive(Debug)]
struct MatchResult {
    path: PathBuf,
    score: u32,
}

enum VcsDisplay {
    BranchName,
    LastCommit,
    ChangeCount,
}

struct UiState {
    query: String,
    results: Vec<MatchResult>,
    selected_index: usize,
    vcs_display: VcsDisplay,
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("no project selected")]
    NothingSelected,
    #[error("failed to render")]
    RenderError(#[from] std::io::Error),
}

impl UiState {
    fn select_next(&mut self) {
        if self.selected_index == self.results.len() - 1 {
            self.selected_index = 0;
        } else {
            self.selected_index += 1;
        }
    }

    fn select_prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.results.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    fn add_char(&mut self, c: char) {
        self.query.push(c);
        self.selected_index = 0;
    }

    fn remove_char(&mut self) {
        self.query.pop();
        self.selected_index = 0;
    }

    fn cycle_vcs_display(&mut self) {
        self.vcs_display = match self.vcs_display {
            VcsDisplay::LastCommit => VcsDisplay::BranchName,
            VcsDisplay::BranchName => VcsDisplay::ChangeCount,
            VcsDisplay::ChangeCount => VcsDisplay::LastCommit,
        }
    }
}

// https://jonasjacek.github.io/colors/
static HIGHLIGHT_BG: Color = Color::White;
static HIGHLIGHT_FG: Color = Color::Black;
static RESULT_FOOTER_FG: Color = Color::AnsiValue(219);

fn truncate_end(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        let mut return_string = String::with_capacity(max_len);

        s.chars()
            .take(max_len - 2)
            .for_each(|c| return_string.push(c));
        return_string += "..";

        return return_string;
    }

    s.to_string()
}

fn truncate_beginning(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        let ignore_count = (s.len() - max_len) + 2;
        let mut return_string = String::with_capacity(max_len);
        return_string += "..";
        s.chars()
            .skip(ignore_count)
            .for_each(|c| return_string.push(c));

        return return_string;
    }

    s.to_string()
}

fn render(query: &str, state: &UiState, cache: &CacheClient) -> std::io::Result<()> {
    let mut stderr = BufWriter::with_capacity(1 << 16, stderr());

    queue!(stderr, terminal::Clear(terminal::ClearType::All))?;

    let (cols, rows) = dimensions_stderr().unwrap();
    let rows = rows as u16;

    let mut row = rows - 3;
    let result_row_count = (rows - 4) as usize;
    let summary_col = (cols / 10) * 6;

    let first_result = state.selected_index.saturating_sub(result_row_count);

    for (i, result) in state.results.iter().enumerate() {
        if i < first_result {
            continue;
        }

        let is_selected = i == state.selected_index;
        let vcs_info = cache.get_vcs_info(&result.path);

        let has_pending_changes =
            matches!(&vcs_info, Some(vcs_info) if vcs_info.uncommitted_changes.unwrap_or(0) > 0);

        if is_selected {
            queue!(
                stderr,
                SetBackgroundColor(HIGHLIGHT_BG),
                SetForegroundColor(match has_pending_changes {
                    true => Color::Red,
                    false => HIGHLIGHT_FG,
                }),
                cursor::MoveTo(0, row),
                Print(match has_pending_changes {
                    true => ">*",
                    false => "> ",
                }),
            )?;
        } else {
            queue!(
                stderr,
                SetBackgroundColor(Color::Reset),
                SetForegroundColor(match has_pending_changes {
                    true => Color::Red,
                    false => Color::Reset,
                }),
                cursor::MoveTo(0, row),
                Print(match has_pending_changes {
                    true => "*",
                    false => " ",
                }),
                SetBackgroundColor(Color::Reset),
            )?;
        }
        queue!(
            stderr,
            cursor::MoveTo(2, row),
            Print(truncate_beginning(
                result.path.to_str().unwrap(),
                summary_col - 3,
            )),
        )?;

        if let Some(vcs_info) = &vcs_info {
            let vcs_summary = match state.vcs_display {
                VcsDisplay::BranchName => vcs_info.current_branch_name.clone(),
                VcsDisplay::LastCommit => vcs_info.last_commit_summary.clone(),
                VcsDisplay::ChangeCount => match vcs_info.uncommitted_changes {
                    Some(count) => format!("{} pending changes", count),
                    None => "counting...".to_string(),
                },
            };

            queue!(
                stderr,
                cursor::MoveTo(summary_col as u16, row),
                Print(truncate_end(&vcs_summary, cols - summary_col)),
            )?;
        }

        queue!(
            stderr,
            SetForegroundColor(Color::Reset),
            SetBackgroundColor(Color::Reset),
        )?;

        if row == 1 {
            break;
        }
        row -= 1;
    }

    let vcs_title = match state.vcs_display {
        VcsDisplay::BranchName => "Branch",
        VcsDisplay::LastCommit => "Last change",
        VcsDisplay::ChangeCount => "Pending changes",
    };
    // Render heading
    queue!(
        stderr,
        cursor::MoveTo(0, 0),
        SetForegroundColor(Color::Black),
        SetBackgroundColor(Color::White),
        Print(format!(
            "{:width_a$}{:width_b$}",
            "  Project",
            format!("{}   <TAB>", vcs_title),
            width_a = { summary_col },
            width_b = cols - summary_col,
        )),
        SetBackgroundColor(Color::Reset),
        SetForegroundColor(Color::Reset)
    )?;

    let prompt_row = rows - 1;
    queue!(
        stderr,
        SetForegroundColor(RESULT_FOOTER_FG),
        cursor::MoveTo(2, prompt_row - 1),
        Print(format!("{}", state.results.len())),
        SetBackgroundColor(HIGHLIGHT_BG),
        SetForegroundColor(Color::Blue),
        cursor::MoveTo(0, prompt_row),
        Print(">"),
        cursor::MoveTo(2, prompt_row),
        Print(query),
        SetBackgroundColor(Color::Reset),
        SetForegroundColor(Color::Reset)
    )?;

    stderr.flush()?;

    Ok(())
}

pub fn run(config: &Config) -> Result<(), CommandError> {
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT.match_paths());

    let cache = Cache::new();

    let mut cache = cache.share();
    let mut cache2 = cache.clone();

    let root = Traverser::new(&config.root, 3);
    thread::spawn(move || {
        for project in root {
            cache2.add_project(project);
        }

        for p in cache2.get_projects() {
            // TODO: Record a failure to read git info for this project
            if let Ok(vcs_info) = fetch_vcs_info(&p.path) {
                cache2.add_vcs_info(&p.path, vcs_info);
            }
        }

        for p in cache2.get_projects() {
            if let Some(mut vcs_info) = cache2.get_vcs_info(&p.path) {
                let count = Repository::open(&p.path).and_then(|repo| {
                    repo.statuses(Some(
                        StatusOptions::new()
                            .include_ignored(false)
                            .include_untracked(true),
                    ))
                    .map(|statuses| statuses.len())
                });
                if let Ok(count) = count {
                    vcs_info.uncommitted_changes = Some(count);
                    cache2.add_vcs_info(&p.path, vcs_info);
                }
            }
        }
    });

    let mut exit = false;
    let mut ui_state = UiState {
        vcs_display: VcsDisplay::LastCommit,
        query: String::from(""),
        results: vec![],
        selected_index: 0,
    };
    let mut selected_project = None;

    execute!(stderr(), terminal::EnterAlternateScreen)?;
    while !exit {
        ui_state.results = Vec::new();
        let pattern = Pattern::parse(&ui_state.query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        for proj in cache.get_projects() {
            let haystack = Utf32Str::new(proj.path.to_str().unwrap(), &mut buf);
            let match_score = pattern.score(haystack, &mut matcher);
            if let Some(score) = match_score {
                ui_state.results.push(MatchResult {
                    score,
                    path: proj.path.to_path_buf(),
                });
            }
        }

        ui_state.results.sort_by_key(|result| Reverse(result.score));

        render(&ui_state.query, &ui_state, &cache)?;

        terminal::enable_raw_mode()?;

        let mut input_available: bool = false;
        while !input_available && !cache.has_new_data() {
            input_available = poll(Duration::from_millis(50)).unwrap();
        }
        terminal::disable_raw_mode()?;

        if input_available {
            let read_result = read();

            match read_result.unwrap() {
                Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                    KeyCode::Char(c) => ui_state.add_char(c),
                    KeyCode::Backspace => ui_state.remove_char(),
                    KeyCode::Down => ui_state.select_prev(),
                    KeyCode::Up => ui_state.select_next(),
                    KeyCode::Esc => exit = true,
                    KeyCode::Tab => ui_state.cycle_vcs_display(),
                    KeyCode::Enter => {
                        selected_project =
                            Some(ui_state.results[ui_state.selected_index].path.clone());
                        exit = true;
                    }
                    _ => (),
                },
                _ => (),
            };
        }
    }
    execute!(stderr(), terminal::LeaveAlternateScreen)?;

    if let Some(path) = selected_project {
        println!("{}", path.to_str().unwrap());
        return Ok(());
    }

    Err(CommandError::NothingSelected)
}
