use crate::input::ScreenId;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub id: String,
    pub label: String,
    pub description: String,
    pub shortcut: Option<String>,
    pub is_destructive: bool,
    pub action: PaletteAction,
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    Navigate(ScreenId),
    RunCommand(String),
}

pub struct CommandPalette {
    pub commands: Vec<PaletteCommand>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub visible: bool,
    matcher: SkimMatcherV2,
}

impl CommandPalette {
    pub fn new(commands: Vec<PaletteCommand>) -> Self {
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self {
            commands,
            query: String::new(),
            filtered,
            selected: 0,
            visible: false,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.refilter();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.refilter();
    }

    pub fn update_query(&mut self, query: String) {
        self.query = query;
        self.refilter();
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn selected_command(&self) -> Option<&PaletteCommand> {
        self.filtered
            .get(self.selected)
            .map(|&idx| &self.commands[idx])
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.commands.len()).collect();
        } else {
            self.filtered = self
                .commands
                .iter()
                .enumerate()
                .filter_map(|(idx, cmd)| {
                    self.matcher
                        .fuzzy_match(&cmd.label, &self.query)
                        .map(|_score| idx)
                })
                .collect();
        }
        self.selected = 0;
    }
}
