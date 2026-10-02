//! 以中查英：英文模式下敲拼音、按中文意思选英文。

use super::*;

/// 中→英释义表的替身：开发 两条义项、咖啡 / 开 / 西安 各一条。
fn glossary() -> Box<dyn Translator> {
    let entry = |senses: &[(PartOfSpeech, &str)]| {
        Translation::new(
            Language::English,
            senses
                .iter()
                .map(|(part_of_speech, text)| Sense {
                    part_of_speech: Some(*part_of_speech),
                    text: (*text).to_owned(),
                    reading: None,
                    fresh: false,
                })
                .collect(),
        )
    };
    let mut table = HashMap::new();
    table.insert(
        "开发".to_owned(),
        entry(&[
            (PartOfSpeech::Verb, "develop"),
            (PartOfSpeech::Verb, "exploit"),
        ]),
    );
    table.insert("咖啡".to_owned(), entry(&[(PartOfSpeech::Noun, "coffee")]));
    table.insert("开".to_owned(), entry(&[(PartOfSpeech::Verb, "open")]));
    table.insert("西安".to_owned(), entry(&[(PartOfSpeech::Noun, "Xi'an")]));
    Box::new(LearningTranslator(table))
}

fn english_engine() -> Engine {
    let mut engine = engine()
        .with_lookup_translator(glossary())
        .with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_english_mode(true);
    engine
}

fn kinds_of(engine: &Engine) -> Vec<CandidateKind> {
    engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .map(|c| c.kind)
        .collect()
}

#[test]
fn clean_pinyin_in_english_mode_brings_english_for_the_chinese_word() {
    let mut engine = english_engine();
    engine.set_input("kaifa");
    let items = engine.query().unwrap().candidates.items;
    let texts: Vec<&str> = items.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(texts, ["develop", "exploit"]);
    assert!(items.iter().all(|c| c.kind == CandidateKind::Translated));
    // 注释是中文词本身，带英文义项的词性
    let translation = items[0].translation.as_ref().unwrap();
    assert_eq!(translation.language, Language::Chinese);
    assert_eq!(translation.senses()[0].text, "开发");
    assert_eq!(
        translation.senses()[0].part_of_speech,
        Some(PartOfSpeech::Verb)
    );
    // xian 也能读成 xi'an，两个完整音节就查
    engine.set_input("xian");
    assert_eq!(texts_of(&engine), ["Xi'an"]);
}

#[test]
fn translated_candidates_sit_between_leading_completions_and_corrections() {
    let words = WordList::parse(
        "kafei\tkafei\t100\nkafeia\tkafeia\t90\nkafeib\tkafeib\t80\nkafeic\tkafeic\t70\n\
         kafeid\tkafeid\t60\nkafej\tkafej\t50\n",
    )
    .unwrap();
    let mut engine = english_engine().with_english(words);
    engine.set_english_mode(true);
    engine.set_input("kafei");
    assert_eq!(
        texts_of(&engine),
        [
            "kafei", "kafeia", "kafeib", "kafeic", "coffee", "kafeid", "kafej"
        ]
    );
}

#[test]
fn single_syllables_need_the_prefix_key() {
    let mut engine = english_engine();
    // kai 只有一个音节，太像英文也太多同音字：不自动查
    engine.set_input("kai");
    assert!(texts_of(&engine).is_empty());
    engine.set_input(";kai");
    assert!(engine.english_lookup_mode());
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "open");
    assert_eq!(query.marked_text(), ";kai");
    // 前缀后面整段只按拼音查，不出英文词表的候选
    engine.set_input(";kaifa");
    assert_eq!(engine.query().unwrap().marked_text(), ";kai'fa");
    assert_eq!(kinds_of(&engine), [CandidateKind::Translated; 2]);
}

#[test]
fn committing_inserts_the_english_word_and_learns_the_choice() {
    let recorded = Arc::new(Mutex::new(HashMap::new()));
    let mut engine =
        english_engine().with_vocabulary_tracker(Box::new(MemoryVocabulary(recorded.clone())));
    engine.set_english_mode(true);
    engine.set_input("kaifa");
    let exploit = engine.query().unwrap().candidates.items[1].clone();
    assert_eq!(engine.commit(&exploit), "exploit");
    assert!(engine.composition().is_empty());
    // 这串拼音下选过 exploit，下次它排第一
    engine.set_input("kaifa");
    assert_eq!(texts_of(&engine), ["exploit", "develop"]);
    // 前缀进来的也一样，吃掉整段（连前缀）；学习键不带前缀，两边共用
    engine.set_input(";kaifa");
    assert_eq!(texts_of(&engine), ["exploit", "develop"]);
    let develop = engine.query().unwrap().candidates.items[1].clone();
    assert_eq!(engine.commit(&develop), "develop");
    assert!(engine.composition().is_empty());
    // 用出去的是英文词，记进英文的词汇；中文注释不当学习词记
    let recorded = recorded.lock().unwrap();
    assert_eq!(
        recorded.get(&(Language::English, "develop".to_owned())),
        Some(&(0, 1, 1))
    );
    assert!(!recorded.keys().any(|(_, word)| word == "开发"));
}

#[test]
fn annotate_keeps_the_chinese_note() {
    let mut engine = english_engine().with_translator(Box::new(FixedTranslator));
    engine.set_english_mode(true);
    engine.set_input("kaifa");
    let mut list = engine.query().unwrap().candidates;
    engine.annotate(&mut list);
    let translation = list.items[0].translation.as_ref().unwrap();
    assert_eq!(translation.language, Language::Chinese);
    assert_eq!(translation.senses()[0].text, "开发");
}

#[test]
fn switches_and_missing_glossary_turn_it_off() {
    // 没装中→英释义表：什么都不出
    let mut bare = engine();
    bare.set_english_mode(true);
    bare.set_input("kaifa");
    assert!(texts_of(&bare).is_empty());
    // 自动混排关掉后前缀键照样能用
    let mut engine = english_engine();
    engine.set_english_lookup(false);
    engine.set_input("kaifa");
    assert!(texts_of(&engine).is_empty());
    engine.set_input(";kaifa");
    assert_eq!(texts_of(&engine), ["develop", "exploit"]);
    // 前缀键关掉：`;` 开头不再是查英文
    engine.set_english_lookup_key(None);
    assert!(!engine.takes_english_lookup_key(';'));
    assert!(!engine.english_lookup_mode());
    assert!(texts_of(&engine).is_empty());
    // 中文模式不受影响
    let mut chinese = english_engine();
    chinese.set_english_mode(false);
    chinese.set_input("kaifa");
    assert_eq!(chinese.query().unwrap().candidates.items[0].text, "开发");
}

#[test]
fn shuangpin_only_looks_up_behind_the_prefix_key() {
    let mut engine = english_engine();
    engine.set_shuangpin(Some(Scheme::Xiaohe));
    // 小鹤 kd fa = kai fa：双拼的键两两都像音节，不自动查
    engine.set_input("kdfa");
    assert!(texts_of(&engine).is_empty());
    engine.set_input(";kdfa");
    assert_eq!(texts_of(&engine), ["develop", "exploit"]);
}

#[test]
fn bare_prefix_key_restores_to_itself() {
    let mut engine = english_engine();
    assert!(engine.takes_english_lookup_key(';'));
    engine.set_input(";");
    assert!(engine.bare_english_lookup());
    assert_eq!(engine.restore_bare_english_lookup().as_deref(), Some(";"));
    assert!(engine.composition().is_empty());
    assert_eq!(engine.restore_bare_english_lookup(), None);
    // 中文模式下缓冲区里的 `;` 不是查英文的入口
    engine.set_english_mode(false);
    engine.set_input(";");
    assert!(!engine.bare_english_lookup());
}

#[test]
fn only_punctuation_outside_english_words_can_be_the_prefix_key() {
    assert!(is_valid_english_lookup_key(';'));
    assert!(is_valid_english_lookup_key('`'));
    for key in ['a', '1', '\'', '-', '_', '?', ' '] {
        assert!(!is_valid_english_lookup_key(key), "{key:?}");
    }
    let mut engine = english_engine();
    engine.set_english_lookup_key(Some('a'));
    assert_eq!(
        engine.english_lookup_key(),
        Some(DEFAULT_ENGLISH_LOOKUP_KEY)
    );
    engine.set_english_lookup_key(Some('`'));
    assert!(engine.takes_english_lookup_key('`'));
}
