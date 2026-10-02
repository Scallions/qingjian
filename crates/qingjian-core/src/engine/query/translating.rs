//! 以中查英：英文模式下把输入当拼音，查出覆盖整段的中文词，按中→英释义表拆成英文候选（`CandidateKind::Translated`）。
//! 设计见 docs/design/candidate-ui.md「以中查英」。

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::Query;
use crate::candidate::{Candidate, CandidateKind, CandidateList, Language, Sense, Translation};
use crate::engine::{
    Engine, Learner, MIN_TRANSLATED_LETTERS, MIN_TRANSLATED_SYLLABLES, TRANSLATED_SCAN,
    TRANSLATED_WORDS, Timings, choice_key,
};
use crate::parser::{self, Segmentation};

impl Engine {
    /// 前缀键开头（`;kaifa`）：只出以中查英候选，单音节、末音节没打完也查。preedit 显示前缀加切分好的拼音。
    pub(in crate::engine) fn query_translated(
        &self,
        scope: &str,
        body: &str,
        rest: String,
        start: Instant,
    ) -> Query {
        let items = self.translated_candidates(body, true);
        let prefix = &scope[..scope.len() - body.len()];
        Query {
            segmentations: Vec::new(),
            candidates: CandidateList { items },
            tail: format!("{prefix}{}", self.marked_rest(body)),
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
                lookup: start.elapsed(),
                rank: Duration::ZERO,
            },
        }
    }

    /// 英文模式里自动混排的以中查英候选：整段是干净的全拼（每个音节都完整、至少两个音节）才查。
    /// 双拼与注音的键几乎两两都能解成音节，打什么英文词都会被当成拼音，那两种方案下只靠前缀键。
    pub(in crate::engine) fn auto_translated(&self, scope: &str) -> Vec<Candidate> {
        if !self.english_lookup
            || self.shuangpin.is_some()
            || self.zhuyin
            || scope.len() < MIN_TRANSLATED_LETTERS
            || !scope.bytes().all(|b| b.is_ascii_lowercase())
            || !parser::is_fully_segmentable(scope)
        {
            return Vec::new();
        }
        self.translated_candidates(scope, false)
    }

    /// 以中查英候选的学习键：去掉前缀、按敲的字母记，查询排序与上屏记学习两边要对得上。
    pub(in crate::engine) fn translated_choice_key(&self, scope: &str) -> String {
        let body = self
            .english_lookup_body(scope)
            .unwrap_or(scope)
            .to_ascii_lowercase();
        choice_key(&body, body.len())
    }

    /// 把 `keys` 当拼音查覆盖整段的中文词，每个词的英文释义各成一条候选，英文按小写去重。
    /// 中文词按选过的次数、词频排；拆出的英文再按「这串拼音下选过这个英文」的次数稳定重排。
    fn translated_candidates(&self, keys: &str, forced: bool) -> Vec<Candidate> {
        if self.lookup_translator.language() != Language::English {
            return Vec::new();
        }
        let keys = keys.to_ascii_lowercase();
        let segmentations: Vec<Segmentation> = match self.decode(&keys) {
            Some(decoded) if decoded.tail().is_empty() => {
                decoded.segmentation().into_iter().collect()
            }
            Some(_) => Vec::new(),
            None => parser::segment(&keys).unwrap_or_default(),
        };
        // 同一个词可能从几种切分、几本词库里命中，留词频最高的一条
        let mut words: HashMap<&str, u32> = HashMap::new();
        for segmentation in segmentations.iter().filter(|s| {
            forced || (s.incomplete_count() == 0 && s.syllables.len() >= MIN_TRANSLATED_SYLLABLES)
        }) {
            let patterns = segmentation.patterns();
            let expanded = self.fuzzy.expand(&patterns);
            for hit in self.lookup_all(&expanded.positions()) {
                if hit.exact {
                    let frequency = words.entry(hit.text).or_default();
                    *frequency = (*frequency).max(hit.frequency);
                }
            }
        }
        let mut words: Vec<(&str, u32)> = words.into_iter().collect();
        words.sort_by(|a, b| {
            self.learner
                .weight(b.0)
                .cmp(&self.learner.weight(a.0))
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.cmp(b.0))
        });
        let glossed = words
            .into_iter()
            .take(TRANSLATED_SCAN)
            .filter_map(|(word, _)| {
                self.lookup_translator
                    .translate(word)
                    .map(|translation| (word, translation))
            })
            .take(TRANSLATED_WORDS);
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for (word, translation) in glossed {
            for sense in translation.senses() {
                let english = sense.text.trim();
                if english.is_empty() || !seen.insert(english.to_ascii_lowercase()) {
                    continue;
                }
                items.push(Candidate {
                    text: english.to_owned(),
                    kind: CandidateKind::Translated,
                    syllables: Vec::new(),
                    reading: None,
                    translation: Some(Translation::new(
                        Language::Chinese,
                        vec![Sense {
                            part_of_speech: sense.part_of_speech,
                            text: word.to_owned(),
                            reading: None,
                            fresh: false,
                        }],
                    )),
                    aux_code: None,
                });
            }
        }
        let input = choice_key(&keys, keys.len());
        items.sort_by_key(|c| Reverse(self.learner.choice_weight(&input, &c.text)));
        items
    }
}
