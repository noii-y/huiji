use qingjian_dictionary::DictionaryError;
use qingjian_learning::LearningError;
use qingjian_lm::LmError;
use qingjian_neural::NeuralError;
use qingjian_platform::ConfigError;
#[cfg(any(
    feature = "local-nmt",
    feature = "local-nmt-mkl",
    feature = "local-nmt-system"
))]
use qingjian_predict::LocalTranslateError;
use qingjian_predict::PredictError;
use qingjian_translate::GlossaryError;
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Cold(#[from] crate::cold::error::ColdError),

    #[error(transparent)]
    Dictionary(#[from] DictionaryError),

    #[error(transparent)]
    Neural(#[from] NeuralError),

    #[error(transparent)]
    Glossary(#[from] GlossaryError),

    #[error(transparent)]
    Learning(#[from] LearningError),

    /// 学习语言不是 en / ja / es。
    #[error("learning language must be en, ja or es, got {0:?}")]
    Language(String),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Predict(#[from] PredictError),

    #[error(transparent)]
    LanguageModel(#[from] LmError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Replay(#[from] crate::replay::ReplayError),

    #[error(transparent)]
    Eval(#[from] crate::eval::EvalError),

    #[error(transparent)]
    Tune(#[from] crate::tuning::TuneError),

    /// 云联想没开或没配密钥，整句翻译发不出去。
    #[error("整句翻译不可用：需要 --predict 且配置可用的云端密钥")]
    TranslationUnavailable,

    /// 等译文超过设定时间。
    #[error("等待整句译文超时（{0} 秒）")]
    TranslationTimeout(u64),

    /// 端侧翻译模型加载或推理失败。
    #[cfg(any(
        feature = "local-nmt",
        feature = "local-nmt-mkl",
        feature = "local-nmt-system"
    ))]
    #[error(transparent)]
    LocalNmt(#[from] LocalTranslateError),

    /// 给了 --local-nmt 但没以 local-nmt feature 编译。
    #[cfg(not(any(
        feature = "local-nmt",
        feature = "local-nmt-mkl",
        feature = "local-nmt-system"
    )))]
    #[error("端侧翻译未编入：请用 --features local-nmt 重新编译 CLI")]
    LocalNmtNotCompiled,
}
