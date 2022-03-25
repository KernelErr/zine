use std::{borrow::Cow, collections::BTreeMap, fs, path::Path};

use anyhow::Result;
use jieba_rs::Jieba;
use serde::{Deserialize, Serialize};
use tera::Context;

use crate::{
    meta::{extract_description_from_markdown, Meta},
    Render,
};

use super::{article::Article, Entity};

/// The season entity config.
/// It parsed from season directory's `zine.toml`.
#[derive(Serialize, Deserialize)]
pub struct Season {
    pub slug: String,
    pub number: u32,
    pub title: String,
    /// The optional introduction for this season.
    pub intro: Option<String>,
    pub cover: Option<String>,
    pub path: String,
    #[serde(rename(deserialize = "article"))]
    #[serde(default)]
    pub articles: Vec<Article>,
    #[serde(skip)]
    pub word_count: BTreeMap<String, u32>,
}

impl std::fmt::Debug for Season {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Season")
            .field("slug", &self.slug)
            .field("number", &self.number)
            .field("title", &self.title)
            .field("intro", &self.intro.is_some())
            .field("cover", &self.cover)
            .field("articles", &self.articles)
            .field("word_count", &self.word_count)
            .finish()
    }
}

impl Season {
    // Get the description of this season.
    // Mainly for html meta description tag.
    fn description(&self) -> String {
        if let Some(intro) = self.intro.as_ref() {
            extract_description_from_markdown(intro)
        } else {
            String::default()
        }
    }

    fn sibling_articles(&self, current: usize) -> (Option<&Article>, Option<&Article>) {
        if current == 0 {
            return (None, self.articles.get(current + 1));
        }

        (
            self.articles.get(current - 1),
            self.articles.get(current + 1),
        )
    }
}

impl Entity for Season {
    fn parse(&mut self, source: &Path) -> Result<()> {
        // Parse intro file
        if let Some(intro_path) = &self.intro {
            self.intro = Some(fs::read_to_string(&source.join(&intro_path))?);
        }

        // Representing a zine.toml file for season.
        #[derive(Debug, Deserialize)]
        struct SeasonFile {
            #[serde(rename = "article")]
            articles: Vec<Article>,
        }

        let dir = source.join(&self.path);
        let content = fs::read_to_string(&dir.join(crate::ZINE_FILE))?;
        let season_file = toml::from_str::<SeasonFile>(&content)?;
        self.articles = season_file.articles;
        // Sort all articles by pub_date.
        self.articles
            .sort_unstable_by_key(|article| article.pub_date);

        self.articles.parse(&dir)?;

        // Analyze words frequencies
        let jieba = Jieba::new();
        let mut word_count: BTreeMap<String, u32> = BTreeMap::new();
        for article in &self.articles {
            let words = jieba.cut(&article.markdown, true);
            for word in words {
                // Count word only if its length is greater than 1
                if word.chars().count() > 1 {
                    let count = word_count.entry(word.to_string()).or_insert(0);
                    *count += 1;
                }
            }
        }
        self.word_count = word_count;

        Ok(())
    }

    fn render(&self, mut context: Context, dest: &Path) -> Result<()> {
        let season_dir = dest.join(&self.slug);
        context.insert("season", &self);

        // Render articles with number context.
        for (index, article) in self.articles.iter().enumerate() {
            let mut context = context.clone();
            context.insert("siblings", &self.sibling_articles(index));
            context.insert("number", &(index + 1));
            article.render(context.clone(), &season_dir.join(article.slug()))?;
        }

        context.insert(
            "meta",
            &Meta {
                title: Cow::Borrowed(&self.title),
                description: Cow::Owned(self.description()),
                url: Some(Cow::Borrowed(&self.slug)),
                image: self.cover.as_deref().map(Cow::Borrowed),
            },
        );
        Render::render("season.jinja", &context, season_dir)?;
        Ok(())
    }
}
