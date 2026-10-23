use super::*;
use fastembed::TextEmbedding;

#[test]
fn default_model_is_bge_small() {
    assert_eq!(SupportedModel::DEFAULT.name(), "bge-small-en-v1.5");
}

#[test]
fn every_model_name_round_trips_through_from_str() {
    for model in SupportedModel::ALL {
        assert_eq!(model.name().parse::<SupportedModel>().unwrap(), model);
        assert_eq!(model.to_string(), model.name());
    }
}

#[test]
fn from_str_is_case_insensitive_and_trims() {
    let model: SupportedModel = "  BGE-Base-EN-v1.5 ".parse().unwrap();
    assert_eq!(model, SupportedModel::BgeBaseEnV15);
}

#[test]
fn from_str_rejects_unknown_name_listing_supported_models() {
    let err = "gpt-4".parse::<SupportedModel>().unwrap_err();
    let Error::EmbeddingError(msg) = err else {
        panic!("expected EmbeddingError");
    };
    assert!(msg.contains("gpt-4"));
    for model in SupportedModel::ALL {
        assert!(msg.contains(model.name()));
    }
}

#[test]
fn dimensions_match_fastembed_model_info() {
    for model in SupportedModel::ALL {
        let embedding_model = model.embedding_model();
        let info = TextEmbedding::get_model_info(&embedding_model).unwrap();
        assert_eq!(model.dimensions(), info.dim, "{}", model.name());
    }
}

#[test]
fn bge_models_use_no_prefixes() {
    for model in [SupportedModel::BgeSmallEnV15, SupportedModel::BgeBaseEnV15] {
        assert_eq!(model.query_prefix(), "");
        assert_eq!(model.passage_prefix(), "");
    }
}

#[test]
fn nomic_and_e5_use_distinct_query_and_passage_prefixes() {
    assert_eq!(
        SupportedModel::NomicEmbedTextV15.query_prefix(),
        "search_query: "
    );
    assert_eq!(
        SupportedModel::NomicEmbedTextV15.passage_prefix(),
        "search_document: "
    );
    assert_eq!(
        SupportedModel::MultilingualE5Small.query_prefix(),
        "query: "
    );
    assert_eq!(
        SupportedModel::MultilingualE5Small.passage_prefix(),
        "passage: "
    );
}

#[test]
fn bge_small_default_threshold_keeps_the_previous_default() {
    assert_eq!(SupportedModel::BgeSmallEnV15.default_threshold(), 0.9);
}

#[test]
fn every_model_has_a_usable_default_threshold() {
    // Vectors are L2-normalised, so distances lie in 0.0..=2.0.
    for model in SupportedModel::ALL {
        let threshold = model.default_threshold();
        assert!(threshold > 0.0 && threshold <= 2.0, "{}", model.name());
    }
}
