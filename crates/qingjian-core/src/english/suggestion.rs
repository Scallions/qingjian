//! 一条英文候选连同它是哪一组出来的：英文模式要把以中查英的候选插在补全与纠正之间。

/// [`super::suggest_tagged`] 的结果，按组区分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suggestion {
    /// 整段正好是的词。
    Exact(String),

    /// 前缀补全。
    Completion(String),

    /// 差一处编辑的纠正。
    Correction(String),
}

impl Suggestion {
    pub fn text(&self) -> &str {
        match self {
            Self::Exact(text) | Self::Completion(text) | Self::Correction(text) => text,
        }
    }

    pub fn into_text(self) -> String {
        match self {
            Self::Exact(text) | Self::Completion(text) | Self::Correction(text) => text,
        }
    }
}
