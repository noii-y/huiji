use qingjian_lm::BigramModel;

fn main() {
    let model = BigramModel::from_path(std::path::Path::new("data/generated/lm.qj")).unwrap();
    let (unigram, bigram) = model.to_tsv();
    std::fs::write("data/generated/lm-unigram.tsv", unigram).unwrap();
    std::fs::write("data/generated/lm-bigram.tsv", bigram).unwrap();
    println!(
        "words={} bigrams={}",
        model.word_count(),
        model.bigram_count()
    );
}
