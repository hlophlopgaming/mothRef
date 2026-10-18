// SPDX-License-Identifier: GPL-3.0-or-later
//! Short-lived presentation copies, bounded independently of board size.
use super::*;
use moth_ui::motion::{Ease, Tween};
const LIMIT: usize = 300;
struct Transition {
    from: Option<Item>,
    to: Option<Item>,
    tween: Tween,
}
#[derive(Default)]
pub(super) struct Items {
    entries: HashMap<Uuid, Transition>,
    suppressed: Option<Instant>,
}
impl Items {
    pub fn remove(&mut self, id: Uuid) {
        self.entries.remove(&id);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn step(&mut self, now: Instant, reduced: bool) -> bool {
        self.entries
            .retain(|_, entry| !reduced && !entry.tween.done(now));
        !self.entries.is_empty()
    }
    pub fn record(&mut self, from: Option<Item>, to: Option<Item>, now: Instant) {
        if self.suppressed.is_some_and(|end| now < end) {
            return;
        }
        if self.entries.len() >= LIMIT {
            self.entries.clear();
            self.suppressed = Some(now + Duration::from_millis(500));
            return;
        }
        // Metadata copies also have a byte budget: oversized notes/names stay instant.
        if from.iter().chain(to.iter()).any(|item| {
            item.name.len() > 4096
                || matches!(&item.content, moth_scene::Content::Text(text) if text.len() > 4096)
        }) {
            return;
        }
        let Some(item) = to.as_ref().or(from.as_ref()) else {
            return;
        };
        let id = item.id;
        let duration = if to.is_none() {
            100
        } else if from.is_none() {
            140
        } else {
            200
        };
        let delay = if from.is_none() {
            (self.entries.len() as u64 * 15).min(300)
        } else {
            0
        };
        self.entries.insert(
            id,
            Transition {
                from,
                to,
                tween: Tween::new(
                    0.,
                    1.,
                    now + Duration::from_millis(delay),
                    Duration::from_millis(duration),
                    Ease::OutCubic,
                ),
            },
        );
    }
    pub fn get(&self, id: Uuid, now: Instant) -> Option<Item> {
        self.entries.get(&id).map(|entry| sample(entry, now))
    }
    pub fn copies(&self, now: Instant) -> Vec<Item> {
        self.entries
            .values()
            .map(|entry| sample(entry, now))
            .collect()
    }
}
fn sample(entry: &Transition, now: Instant) -> Item {
    let t = entry.tween.value(now);
    let mut item = entry
        .to
        .as_ref()
        .or(entry.from.as_ref())
        .expect("transitions always contain an endpoint")
        .clone();
    match (&entry.from, &entry.to) {
        (Some(from), Some(to)) => {
            item.rect = Rect {
                x: from.rect.x + (to.rect.x - from.rect.x) * t,
                y: from.rect.y + (to.rect.y - from.rect.y) * t,
                width: from.rect.width + (to.rect.width - from.rect.width) * t,
                height: from.rect.height + (to.rect.height - from.rect.height) * t,
            };
        }
        (None, Some(_)) => {
            let scale = 0.96 + 0.04 * t;
            item.rect.x += item.rect.width * (1. - scale) / 2.;
            item.rect.y += item.rect.height * (1. - scale) / 2.;
            item.rect.width *= scale;
            item.rect.height *= scale;
            item.transform.opacity *= t;
        }
        (Some(_), None) => item.transform.opacity *= 1. - t,
        _ => unreachable!("transitions always contain an endpoint"),
    }
    item
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_are_bounded_and_expire_with_exact_endpoints() {
        let now = Instant::now();
        let mut items = Items::default();
        for _ in 0..=LIMIT {
            items.record(
                None,
                Some(Item::text(
                    "note",
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: 10.,
                        height: 10.,
                    },
                    None,
                )),
                now,
            );
        }
        assert!(items.entries.is_empty());
        let source = Item::text(
            "note",
            Rect {
                x: 0.,
                y: 0.,
                width: 10.,
                height: 10.,
            },
            None,
        );
        items.suppressed = None;
        items.record(None, Some(source.clone()), now);
        assert_eq!(items.get(source.id, now).unwrap().transform.opacity, 0.);
        assert_eq!(
            items.get(source.id, now + Duration::from_secs(1)),
            Some(source)
        );
        assert!(!items.step(now + Duration::from_secs(1), false));
        items.record(
            None,
            Some(Item::text(
                "note",
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 10.,
                    height: 10.,
                },
                None,
            )),
            now,
        );
        assert!(!items.step(now, true));
    }
}
