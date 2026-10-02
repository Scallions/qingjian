//! 以中查英的开关与前缀键：英文模式下敲拼音、按中文意思挑英文词。候选怎么出见 `query::translating`。

use super::{Engine, Translator};

/// 以中查英前缀键的缺省值（`[general] english_lookup_key`）。
pub const DEFAULT_ENGLISH_LOOKUP_KEY: char = ';';

/// 前缀键校验：ASCII 标点，且不是英文模式里本来就进缓冲区的 `'` `-` `_`，也不是问字的 `?`。设置页与配置加载共用。
pub fn is_valid_english_lookup_key(key: char) -> bool {
    key.is_ascii_graphic() && !key.is_ascii_alphanumeric() && !matches!(key, '\'' | '-' | '_' | '?')
}

impl Engine {
    /// 接以中查英用的释义表（中→英）。与学习语言的表分开：学习语言是日语或关掉译文时也要能查英文。
    pub fn with_lookup_translator(mut self, translator: Box<dyn Translator>) -> Self {
        self.lookup_translator = translator;
        self
    }

    pub fn set_lookup_translator(&mut self, translator: Box<dyn Translator>) {
        self.lookup_translator = translator;
    }

    /// 英文模式下整段像拼音时自动带出以中查英候选（配置 `[general] english_lookup`，缺省开）。前缀键不受它管。
    pub fn set_english_lookup(&mut self, on: bool) {
        self.english_lookup = on;
    }

    pub fn english_lookup(&self) -> bool {
        self.english_lookup
    }

    /// 以中查英的前缀键（配置 `[general] english_lookup_key`），`None` 关掉；不合法的退回缺省。
    pub fn set_english_lookup_key(&mut self, key: Option<char>) {
        self.english_lookup_key = match key {
            Some(key) if !is_valid_english_lookup_key(key) => Some(DEFAULT_ENGLISH_LOOKUP_KEY),
            key => key,
        };
    }

    pub fn english_lookup_key(&self) -> Option<char> {
        self.english_lookup_key
    }

    /// 英文模式、缓冲区为空时敲下 `c` 该不该收进缓冲区当以中查英的前缀：壳据此决定它是入口还是标点。
    pub fn takes_english_lookup_key(&self, c: char) -> bool {
        self.english_lookup_key == Some(c)
    }

    /// 缓冲区以前缀键开头：后面整段按拼音查英文。
    pub fn english_lookup_mode(&self) -> bool {
        self.english_mode && self.english_lookup_body(self.composition.text()).is_some()
    }

    /// 前缀之后的部分；不是以前缀开头时为 `None`。
    pub(super) fn english_lookup_body<'a>(&self, text: &'a str) -> Option<&'a str> {
        let key = self.english_lookup_key?;
        text.strip_prefix(key)
    }

    /// 缓冲区里只有一个前缀键：还没决定是查英文还是标点。
    pub fn bare_english_lookup(&self) -> bool {
        self.english_mode && self.english_lookup_body(self.composition.text()) == Some("")
    }

    /// 确认单独的前缀键是标点：清空缓冲区，原样返回它（英文模式下一律半角）。
    /// 不是单独的前缀键时返回 `None`，不改变组句。
    pub fn restore_bare_english_lookup(&mut self) -> Option<String> {
        if !self.bare_english_lookup() {
            return None;
        }
        let key = self.english_lookup_key?;
        self.clear();
        self.note_passthrough(key);
        Some(key.to_string())
    }
}
