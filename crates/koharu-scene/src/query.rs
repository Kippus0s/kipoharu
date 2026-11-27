//! Reads for tools. Answers serialize the project's own values, so a tool
//! sees a layer as the file stores it.

use std::collections::BTreeMap;

use imbl::ordmap::DiffItem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    Background, Content, History, Layer, LayerId, Page, PageId, Project, ProjectId, Result,
    Session, Size, project::depth_first,
};

#[derive(Clone, Debug, Serialize, Deserialize, Type, JsonSchema)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    /// List the pages in order, with their sizes, backgrounds, and how many
    /// of their blocks are translated.
    GetProject,
    /// Get one page with its layers, each holding its children, masks among
    /// them.
    GetPage { page: PageId },
    /// Get one layer with everything inside it.
    GetLayer { layer: LayerId },
    /// List the texts a reader sees on a page, in reading order: each block
    /// and text layer that shows.
    GetStory { page: PageId },
    /// Get the undo and redo labels.
    GetHistory,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(untagged)]
pub enum Answer {
    Project(ProjectAnswer),
    Page(Page),
    Layer(Box<Layer>),
    Story(Vec<LayerId>),
    History(History),
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct ProjectAnswer {
    pub id: ProjectId,
    /// The format version the file holds.
    pub version: String,
    pub pages: Vec<PageSummary>,
}

/// A page without its layers, which `get_page` answers, and how far its
/// translation is, as a CAT tool's file list shows each file's progress,
/// Trados Studio's Files view among them.
#[derive(Clone, Debug, Serialize, Type)]
pub struct PageSummary {
    pub id: PageId,
    pub size: Size,
    pub background: Option<Background>,
    /// Its blocks, hidden ones too.
    pub texts: usize,
    /// Its blocks whose translation isn't blank. A cleared translation keeps
    /// its style, and styling an untranslated block makes an empty one, so
    /// neither counts.
    pub translated: usize,
}

impl PageSummary {
    fn from_page(page: &Page) -> Self {
        let mut texts = 0;
        let mut translated = 0;
        for layer in depth_first(&page.layers) {
            if let Content::Block(block) = &layer.content {
                texts += 1;
                // A concatenation is blank exactly when every span is blank.
                // No need to allocate the whole translation to test that.
                if block.translation.as_ref().is_some_and(|text| {
                    text.content
                        .0
                        .iter()
                        .any(|span| !span.insert.trim().is_empty())
                }) {
                    translated += 1;
                }
            }
        }
        Self {
            id: page.id,
            size: page.size,
            background: page.background,
            texts,
            translated,
        }
    }
}

/// Derived only from page metadata, not raster tiles or history snapshots.
/// Keep one summary per live page and update it for the persistent map's diff.
#[derive(Default)]
pub(crate) struct PageSummaries(BTreeMap<PageId, PageSummary>);

impl PageSummaries {
    pub(crate) fn new(project: &Project) -> Self {
        Self(
            project
                .pages()
                .map(|page| (page.id, PageSummary::from_page(page)))
                .collect(),
        )
    }

    pub(crate) fn update(&mut self, before: &Project, after: &Project) {
        for item in before.pages.diff(&after.pages) {
            match item {
                DiffItem::Add(id, page) => {
                    self.0.insert(*id, PageSummary::from_page(page));
                }
                DiffItem::Update { new, .. } => {
                    self.0.insert(*new.0, PageSummary::from_page(new.1));
                }
                DiffItem::Remove(id, _) => {
                    self.0.remove(id);
                }
            }
        }
    }

    fn ordered(&self, project: &Project) -> Vec<PageSummary> {
        project
            .order
            .iter()
            .map(|id| self.0[id].clone())
            .collect()
    }
}

impl Session {
    pub fn query(&self, query: Query) -> Result<Answer> {
        let project = self.project();
        Ok(match query {
            Query::GetProject => Answer::Project(ProjectAnswer {
                id: project.id(),
                version: self.version()?,
                pages: self.page_summaries.ordered(&project),
            }),
            Query::GetPage { page } => Answer::Page(project.page(page)?.clone()),
            Query::GetLayer { layer } => Answer::Layer(Box::new(project.layer(layer)?.clone())),
            Query::GetStory { page } => Answer::Story(project.story(page)?),
            Query::GetHistory => Answer::History(self.history()),
        })
    }
}
