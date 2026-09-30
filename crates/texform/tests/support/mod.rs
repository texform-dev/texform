use texform::KnowledgeBase;

/// Build a knowledge base that loads exactly `packages`.
pub fn kb(packages: &[&str]) -> KnowledgeBase {
    KnowledgeBase::builder().packages(packages).build().unwrap()
}
