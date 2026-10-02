//! 拼音以外的几种查询：表达式（`v`）、原样字母、英文模式、问字（`?`）。

use super::*;

impl Engine {
    /// 表达式模式（`v` 开头）：不解析拼音，候选是算式结果 / 中文数字，再加上整段是英文词的情况（`very`）。
    /// preedit 原样显示输入。
    pub(in crate::engine) fn query_expression(
        &self,
        scope: &str,
        rest: String,
        start: Instant,
    ) -> Query {
        let mut items = shortcut::candidates(scope, self.modes().expression, &jiff::Zoned::now());
        if let Some(word) = self.english.as_ref().and_then(|english| english.get(scope)) {
            items.push(Candidate {
                text: word.to_owned(),
                kind: CandidateKind::English,
                syllables: Vec::new(),
                reading: None,
                translation: None,
                aux_code: None,
            });
        }
        Query {
            segmentations: Vec::new(),
            candidates: CandidateList { items },
            tail: scope.to_owned(),
            text: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            rest,
            decoded_keys: self.shuangpin.is_some() || self.zhuyin,
            shuangpin_raw_preedit: false,
            typed_display: None,
            correction: None,
            aux: None,
            timings: Timings {
                parse: Duration::ZERO,
                lookup: Duration::ZERO,
                rank: start.elapsed(),
            },
        }
    }

    /// 英文直输段：唯一候选就是原文（`no-way`），空格 / 回车都上屏它；preedit 原样显示。
    pub(in crate::engine) fn query_raw(&self, scope: &str, rest: String, start: Instant) -> Query {
        let items = vec![Candidate {
            text: scope.to_owned(),
            kind: CandidateKind::English,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        }];
        Query {
            segmentations: Vec::new(),
            candidates: CandidateList { items },
            tail: scope.to_owned(),
            text: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            rest,
            decoded_keys: self.shuangpin.is_some() || self.zhuyin,
            shuangpin_raw_preedit: false,
            typed_display: None,
            correction: None,
            aux: None,
            timings: Timings {
                parse: Duration::ZERO,
                lookup: Duration::ZERO,
                rank: start.elapsed(),
            },
        }
    }

    /// 英文模式：敲的字母原样显示，候选是英文词表的精确词、前缀补全与拼错纠正（见 [`english::suggest`]），
    /// 词表没装就没有候选。整段是干净的全拼时以中查英的候选插在前几条补全之后、其余补全与纠正之前
    /// （`kaifa` → develop / exploit）；前缀键开头的整段只查英文（见 [`Self::query_translated`]）。
    /// emoji 照配，但排在所有词后面：选词靠上下键，emoji 夹在词中间会挡路。
    pub(in crate::engine) fn query_english(
        &self,
        scope: &str,
        rest: String,
        start: Instant,
    ) -> Query {
        if let Some(body) = self.english_lookup_body(scope) {
            return self.query_translated(scope, body, rest, start);
        }
        let english_candidate = |text| Candidate {
            text,
            kind: CandidateKind::English,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        };
        let mut translated = Some(self.auto_translated(scope));
        let mut items: Vec<Candidate> = Vec::new();
        let mut completions = 0;
        for suggestion in english::suggest_tagged(
            &self.english_lists(),
            scope,
            |text| self.learner.weight(text),
            ENGLISH_MODE_CANDIDATES,
        ) {
            let make_room = match suggestion {
                english::Suggestion::Exact(_) => false,
                english::Suggestion::Completion(_) => {
                    completions += 1;
                    completions > ENGLISH_COMPLETIONS
                }
                english::Suggestion::Correction(_) => true,
            };
            if make_room && let Some(translated) = translated.take() {
                items.extend(translated);
            }
            items.push(english_candidate(suggestion.into_text()));
        }
        items.extend(translated.into_iter().flatten());
        self.insert_emoji(&mut items);
        items.sort_by_key(|c| c.kind == CandidateKind::Emoji);
        Query {
            segmentations: Vec::new(),
            candidates: CandidateList { items },
            tail: scope.to_owned(),
            text: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            rest,
            decoded_keys: self.shuangpin.is_some() || self.zhuyin,
            shuangpin_raw_preedit: false,
            typed_display: None,
            correction: None,
            aux: None,
            timings: Timings {
                parse: Duration::ZERO,
                lookup: Duration::ZERO,
                rank: start.elapsed(),
            },
        }
    }

    /// 问字模式（问字键或 `?` 开头）：拼音问题本地没有候选，preedit 显示前缀加切分好的问题拼音，答案等云端；
    /// 十六进制码点（`u4e00`、`u+1f600`）本地直接给出那个字符。
    pub(in crate::engine) fn query_question(
        &self,
        scope: &str,
        rest: String,
        start: Instant,
    ) -> Query {
        let body = self.modes().question_body(scope, self.zhuyin);
        let prefix = &scope[..scope.len() - body.len()];
        let (candidates, tail) = match shortcut::unicode_form(body) {
            Some(text) => (
                CandidateList {
                    items: vec![Candidate {
                        text,
                        kind: CandidateKind::Shortcut,
                        syllables: Vec::new(),
                        reading: None,
                        translation: None,
                        aux_code: None,
                    }],
                },
                scope.to_owned(),
            ),
            None => (
                CandidateList::default(),
                format!("{prefix}{}", self.marked_rest(body)),
            ),
        };
        Query {
            segmentations: Vec::new(),
            candidates,
            tail,
            text: self.composition.text().to_owned(),
            cursor: self.composition.cursor(),
            rest,
            decoded_keys: self.shuangpin.is_some() || self.zhuyin,
            shuangpin_raw_preedit: false,
            typed_display: None,
            correction: None,
            aux: None,
            timings: Timings {
                parse: start.elapsed(),
                lookup: Duration::ZERO,
                rank: Duration::ZERO,
            },
        }
    }
}
