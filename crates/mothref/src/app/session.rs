// SPDX-License-Identifier: GPL-3.0-or-later
//! Timed drawing sessions (gesture mode): what to show, in which order, for how long.
//!
//! Pure state driven by `Instant`s passed in, so the clock is testable and an idle
//! session costs nothing between repaints. The screens live in `session_ui.rs`.
use moth_scene::Uuid;
use std::time::{Duration, Instant};

/// Per-image times offered as one-tap choices, in seconds.
pub(super) const INTERVALS: [u32; 5] = [30, 60, 120, 300, 600];
/// The 3-2-1 before the first image.
pub(super) const COUNTDOWN: Duration = Duration::from_secs(3);
/// Seconds before a change in which the timer turns to the warning color and ticks.
pub(super) const WARN_SECONDS: u32 = 5;
/// Time one "+30 s" press adds to the current image.
pub(super) const EXTRA_SECONDS: u32 = 30;
/// A session never deals more slides than this, whatever the plan says.
const MAX_SLIDES: usize = 10_000;
/// Longest accepted time per image (one hour) and stage size, bounding plans read back
/// from preferences.
const MAX_SECONDS: u32 = 3600;
const MAX_COUNT: u32 = 999;

/// Where the images come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Board,
    Selection,
    Group(Uuid),
}
/// A run of `count` images shown `seconds` each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stage {
    pub count: u32,
    pub seconds: u32,
}
/// How images are dealt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Order {
    /// Shuffled, each image once before any repeats.
    Shuffle,
    /// Reading order on the board: rows top to bottom, left to right.
    Board,
    /// Independent random picks; an image may come back soon.
    Random,
}
/// Everything the setup screen chooses.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Setup {
    pub source: Source,
    /// Time per image when the plan is empty: every image once.
    pub interval: u32,
    pub plan: Vec<Stage>,
    pub order: Order,
    pub sound: bool,
    pub countdown: bool,
    pub warn: bool,
    pub hide: bool,
    pub mini: bool,
}
impl Default for Setup {
    fn default() -> Self {
        Self {
            source: Source::Board,
            interval: 30,
            plan: Vec::new(),
            order: Order::Shuffle,
            sound: true,
            countdown: true,
            warn: true,
            hide: true,
            mini: false,
        }
    }
}
impl Setup {
    /// The stages that actually run for `images` images.
    pub fn stages(&self, images: usize) -> Vec<Stage> {
        if self.plan.is_empty() {
            vec![Stage {
                count: images.min(MAX_SLIDES) as u32,
                seconds: self.interval,
            }]
        } else {
            self.plan.clone()
        }
    }
    /// Number of slides and total planned time.
    pub fn total(&self, images: usize) -> (usize, Duration) {
        if images == 0 {
            return (0, Duration::ZERO);
        }
        self.stages(images)
            .iter()
            .fold((0, Duration::ZERO), |(count, time), stage| {
                (
                    count + stage.count as usize,
                    time + Duration::from_secs(stage.count as u64 * stage.seconds as u64),
                )
            })
    }
    /// Compact preference encoding; the source is not remembered, it follows the board.
    pub fn encode(&self) -> String {
        let order = match self.order {
            Order::Shuffle => 's',
            Order::Board => 'b',
            Order::Random => 'r',
        };
        let flags: String = [self.sound, self.countdown, self.warn, self.hide, self.mini]
            .iter()
            .map(|flag| if *flag { '1' } else { '0' })
            .collect();
        format!(
            "{}|{}|{order}|{flags}",
            self.interval,
            encode_plan(&self.plan)
        )
    }
    /// Reads [`Self::encode`]; anything malformed falls back to the defaults.
    pub fn decode(value: &str) -> Self {
        let mut setup = Self::default();
        let parts: Vec<_> = value.split('|').collect();
        let [interval, plan, order, flags] = parts.as_slice() else {
            return setup;
        };
        if let Ok(interval) = interval.parse::<u32>()
            && (1..=MAX_SECONDS).contains(&interval)
        {
            setup.interval = interval;
        }
        setup.plan = decode_plan(plan).unwrap_or_default();
        setup.order = match *order {
            "b" => Order::Board,
            "r" => Order::Random,
            _ => Order::Shuffle,
        };
        let flags: Vec<bool> = flags.chars().map(|c| c == '1').collect();
        if let [sound, countdown, warn, hide, mini] = flags.as_slice() {
            (
                setup.sound,
                setup.countdown,
                setup.warn,
                setup.hide,
                setup.mini,
            ) = (*sound, *countdown, *warn, *hide, *mini);
        }
        setup
    }
}
/// `10x30,5x120`: stages as count × seconds.
pub(super) fn encode_plan(plan: &[Stage]) -> String {
    plan.iter()
        .map(|stage| format!("{}x{}", stage.count, stage.seconds))
        .collect::<Vec<_>>()
        .join(",")
}
/// Reads [`encode_plan`]; `None` when any stage is out of range.
pub(super) fn decode_plan(value: &str) -> Option<Vec<Stage>> {
    if value.is_empty() {
        return Some(Vec::new());
    }
    value
        .split(',')
        .take(32)
        .map(|stage| {
            let (count, seconds) = stage.split_once('x')?;
            let stage = Stage {
                count: count.parse().ok()?,
                seconds: seconds.parse().ok()?,
            };
            ((1..=MAX_COUNT).contains(&stage.count) && (1..=MAX_SECONDS).contains(&stage.seconds))
                .then_some(stage)
        })
        .collect()
}
/// One shown (or planned) image of a session.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Slide {
    pub item: Uuid,
    pub stage: usize,
    pub seconds: u32,
    /// Time the image was actually on screen, pauses excluded.
    pub spent: Duration,
    /// Seconds added with "+30 s".
    pub extra: u32,
    /// Left before its time ran out.
    pub skipped: bool,
    pub shown: bool,
    /// Marked for a repeat on the summary.
    pub star: bool,
}
impl Slide {
    fn new(item: Uuid, stage: usize, seconds: u32) -> Self {
        Self {
            item,
            stage,
            seconds,
            spent: Duration::ZERO,
            extra: 0,
            skipped: false,
            shown: false,
            star: false,
        }
    }
    fn length(&self) -> Duration {
        Duration::from_secs((self.seconds + self.extra) as u64)
    }
}
/// Small deterministic generator; sessions need variety, not cryptography.
pub(super) struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            items.swap(index, self.below(index + 1));
        }
    }
}
/// Deals the plan over `images` (already in board reading order).
pub(super) fn deal(images: &[Uuid], stages: &[Stage], order: Order, rng: &mut Rng) -> Vec<Slide> {
    let mut slides = Vec::new();
    if images.is_empty() {
        return slides;
    }
    let mut deck: Vec<Uuid> = Vec::new();
    let mut next = 0;
    for (stage_index, stage) in stages.iter().enumerate() {
        for _ in 0..stage.count {
            if slides.len() >= MAX_SLIDES {
                return slides;
            }
            let item = match order {
                Order::Random => images[rng.below(images.len())],
                Order::Board | Order::Shuffle => {
                    if next == deck.len() {
                        let last = deck.last().copied();
                        deck = images.to_vec();
                        if order == Order::Shuffle {
                            rng.shuffle(&mut deck);
                            // A fresh round never repeats the image just shown.
                            if deck.len() > 1 && deck.first().copied() == last {
                                deck.swap(0, 1);
                            }
                        }
                        next = 0;
                    }
                    next += 1;
                    deck[next - 1]
                }
            };
            slides.push(Slide::new(item, stage_index, stage.seconds));
        }
    }
    slides
}
/// What "again" repeats from a finished session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Replay {
    /// The recorded sequence with its times.
    Same,
    /// The same images and times, in a new order.
    Reshuffle,
    /// Only images left early, with their original times.
    Skipped,
    /// Only starred images, with their original times.
    Starred,
}
/// Slides for a repeat of `slides`, dropping images no longer on the board.
pub(super) fn replay(
    slides: &[Slide],
    kind: Replay,
    exists: impl Fn(Uuid) -> bool,
    rng: &mut Rng,
) -> Vec<Slide> {
    let shown = slides.iter().filter(|s| s.shown && exists(s.item));
    let mut out: Vec<Slide> = match kind {
        Replay::Same | Replay::Reshuffle => shown
            .map(|s| Slide::new(s.item, s.stage, s.seconds))
            .collect(),
        Replay::Skipped => shown
            .filter(|s| s.skipped)
            .map(|s| Slide::new(s.item, s.stage, s.seconds))
            .collect(),
        Replay::Starred => shown
            .filter(|s| s.star)
            .map(|s| Slide::new(s.item, s.stage, s.seconds))
            .collect(),
    };
    if kind == Replay::Reshuffle {
        // Times stay with their positions so the stage rhythm repeats.
        let mut items: Vec<_> = out.iter().map(|s| s.item).collect();
        rng.shuffle(&mut items);
        for (slide, item) in out.iter_mut().zip(items) {
            slide.item = item;
        }
    }
    out
}
/// Where the clock is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    /// 3-2-1 until this instant.
    Countdown(Instant),
    Running {
        since: Instant,
        left: Duration,
    },
    Paused {
        left: Duration,
    },
    Done,
}
/// Something the user should hear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cue {
    /// A new image is up.
    Change,
    /// One of the last seconds of an image.
    Tick,
}
/// A session in progress or just finished.
pub(super) struct Run {
    pub setup: Setup,
    pub slides: Vec<Slide>,
    pub index: usize,
    pub phase: Phase,
    /// Shown under the title: where the images came from.
    pub source: String,
    last_tick: Option<u64>,
}
impl Run {
    /// Starts with the countdown when the setup asks for it. `slides` is not empty.
    pub fn new(setup: Setup, mut slides: Vec<Slide>, source: String, now: Instant) -> Self {
        if let Some(first) = slides.first_mut() {
            first.shown = true;
        }
        let phase = match slides.first() {
            None => Phase::Done,
            Some(_) if setup.countdown => Phase::Countdown(now + COUNTDOWN),
            Some(first) => Phase::Running {
                since: now,
                left: first.length(),
            },
        };
        Self {
            setup,
            slides,
            index: 0,
            phase,
            source,
            last_tick: None,
        }
    }
    pub fn current(&self) -> Option<&Slide> {
        self.slides.get(self.index)
    }
    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }
    /// Time left on the current image (the countdown while counting down).
    pub fn left(&self, now: Instant) -> Duration {
        match self.phase {
            Phase::Countdown(end) => end.saturating_duration_since(now),
            Phase::Running { since, left } => {
                left.saturating_sub(now.saturating_duration_since(since))
            }
            Phase::Paused { left } => left,
            Phase::Done => Duration::ZERO,
        }
    }
    /// Share of the current image's time still left, for the timer ring.
    pub fn fraction(&self, now: Instant) -> f32 {
        let total = match self.phase {
            Phase::Countdown(_) => COUNTDOWN,
            _ => self.current().map_or(Duration::ZERO, Slide::length),
        };
        if total.is_zero() {
            return 0.;
        }
        (self.left(now).as_secs_f32() / total.as_secs_f32()).clamp(0., 1.)
    }
    /// The last seconds of a running image, when the setup warns.
    pub fn warning(&self, now: Instant) -> bool {
        self.setup.warn
            && matches!(self.phase, Phase::Running { .. })
            && self.left(now) <= Duration::from_secs(WARN_SECONDS as u64)
    }
    /// Advances the clock; returns what to play.
    pub fn update(&mut self, now: Instant) -> Vec<Cue> {
        let mut cues = Vec::new();
        if let Phase::Countdown(end) = self.phase
            && now >= end
        {
            self.phase = Phase::Running {
                since: end,
                left: self.current().map_or(Duration::ZERO, Slide::length),
            };
            cues.push(Cue::Change);
        }
        // Loops so a long stall (a suspended laptop) catches up without skipping
        // the bookkeeping of the images it passed.
        while let Phase::Running { since, left } = self.phase {
            let elapsed = now.saturating_duration_since(since);
            if elapsed < left {
                let seconds = (left - elapsed).as_secs_f32().ceil() as u64;
                if self.setup.warn
                    && seconds <= WARN_SECONDS as u64
                    && self.last_tick != Some(seconds)
                {
                    self.last_tick = Some(seconds);
                    cues.push(Cue::Tick);
                }
                break;
            }
            if let Some(slide) = self.slides.get_mut(self.index) {
                slide.spent += left;
            }
            if self.index + 1 >= self.slides.len() {
                self.phase = Phase::Done;
                break;
            }
            self.index += 1;
            self.slides[self.index].shown = true;
            self.last_tick = None;
            self.phase = Phase::Running {
                since: since + left,
                left: self.slides[self.index].length(),
            };
            cues.push(Cue::Change);
        }
        cues
    }
    /// Books the time spent so far on the current image and freezes the clock.
    fn hold(&mut self, now: Instant) -> Duration {
        let left = self.left(now);
        if let Phase::Running { since, .. } = self.phase
            && let Some(slide) = self.slides.get_mut(self.index)
        {
            slide.spent += now.saturating_duration_since(since).min(slide.length());
        }
        left
    }
    pub fn toggle_pause(&mut self, now: Instant) {
        self.phase = match self.phase {
            Phase::Running { .. } => Phase::Paused {
                left: self.hold(now),
            },
            Phase::Paused { left } => Phase::Running { since: now, left },
            Phase::Countdown(_) => {
                return self.skip_countdown(now);
            }
            Phase::Done => Phase::Done,
        };
    }
    /// Space during the 3-2-1 starts at once.
    pub fn skip_countdown(&mut self, now: Instant) {
        if let Phase::Countdown(_) = self.phase {
            self.phase = Phase::Countdown(now);
        }
    }
    /// Manual previous/next: books the current image, moves and pauses there.
    /// Returns whether an image changed.
    pub fn step(&mut self, forward: bool, now: Instant) -> bool {
        if self.done() {
            return false;
        }
        let target = if forward {
            self.index + 1
        } else {
            match self.index.checked_sub(1) {
                Some(index) => index,
                None => return false,
            }
        };
        self.hold(now);
        if forward && let Some(slide) = self.slides.get_mut(self.index) {
            slide.skipped |= slide.spent < Duration::from_secs(slide.seconds as u64);
        }
        if target >= self.slides.len() {
            self.phase = Phase::Done;
            return false;
        }
        self.index = target;
        self.last_tick = None;
        let slide = &mut self.slides[target];
        slide.shown = true;
        self.phase = Phase::Paused {
            left: slide.length().saturating_sub(slide.spent),
        };
        true
    }
    /// "+30 s" on the current image.
    pub fn extend(&mut self) {
        let add = Duration::from_secs(EXTRA_SECONDS as u64);
        match &mut self.phase {
            Phase::Running { left, .. } | Phase::Paused { left } => *left += add,
            _ => return,
        }
        if let Some(slide) = self.slides.get_mut(self.index) {
            slide.extra += EXTRA_SECONDS;
        }
        self.last_tick = None;
    }
    /// Ends early: the current image counts as skipped when its time was not up.
    pub fn finish(&mut self, now: Instant) {
        if self.done() {
            return;
        }
        self.hold(now);
        if let Some(slide) = self.slides.get_mut(self.index) {
            slide.skipped |= slide.spent < Duration::from_secs(slide.seconds as u64);
        }
        self.phase = Phase::Done;
    }
    /// Images shown and the time they took.
    pub fn summary(&self) -> (usize, Duration, usize) {
        self.slides
            .iter()
            .filter(|s| s.shown)
            .fold((0, Duration::ZERO, 0), |(count, time, skipped), s| {
                (count + 1, time + s.spent, skipped + s.skipped as usize)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<Uuid> {
        (0..count).map(|_| Uuid::now_v7()).collect()
    }

    #[test]
    fn shuffle_shows_every_image_once_per_round() {
        let images = ids(5);
        let stages = [Stage {
            count: 10,
            seconds: 30,
        }];
        let slides = deal(&images, &stages, Order::Shuffle, &mut Rng::new(7));
        assert_eq!(slides.len(), 10);
        for round in slides.chunks(5) {
            let mut seen: Vec<_> = round.iter().map(|s| s.item).collect();
            seen.sort();
            let mut expected = images.clone();
            expected.sort();
            assert_eq!(seen, expected);
        }
        assert!(slides.windows(2).all(|pair| pair[0].item != pair[1].item));
        let ordered = deal(&images, &stages, Order::Board, &mut Rng::new(7));
        assert_eq!(ordered[6].item, images[1]);
    }

    #[test]
    fn clock_changes_images_ticks_and_books_time() {
        let start = Instant::now();
        let setup = Setup {
            countdown: true,
            ..Setup::default()
        };
        let images = ids(2);
        let slides = deal(
            &images,
            &[Stage {
                count: 2,
                seconds: 10,
            }],
            Order::Board,
            &mut Rng::new(1),
        );
        let mut run = Run::new(setup, slides, String::new(), start);
        assert_eq!(run.update(start + COUNTDOWN), vec![Cue::Change]);
        let begin = start + COUNTDOWN;
        assert!(run.update(begin + Duration::from_secs(4)).is_empty());
        assert_eq!(
            run.update(begin + Duration::from_millis(5500)),
            vec![Cue::Tick]
        );
        assert!(run.update(begin + Duration::from_millis(5900)).is_empty());
        assert!(run.warning(begin + Duration::from_secs(6)));
        // A stall past both images still books both and ends the session.
        run.update(begin + Duration::from_secs(60));
        assert!(run.done());
        assert!(
            run.slides
                .iter()
                .all(|s| s.shown && s.spent == Duration::from_secs(10))
        );
    }

    #[test]
    fn manual_steps_pause_and_mark_skips() {
        let start = Instant::now();
        let setup = Setup {
            countdown: false,
            ..Setup::default()
        };
        let slides = deal(
            &ids(3),
            &[Stage {
                count: 3,
                seconds: 30,
            }],
            Order::Board,
            &mut Rng::new(1),
        );
        let mut run = Run::new(setup, slides, String::new(), start);
        assert!(run.step(true, start + Duration::from_secs(12)));
        assert!(run.slides[0].skipped);
        assert_eq!(run.slides[0].spent, Duration::from_secs(12));
        assert!(matches!(run.phase, Phase::Paused { .. }));
        run.extend();
        assert_eq!(run.left(start), Duration::from_secs(60));
        run.toggle_pause(start + Duration::from_secs(20));
        assert!(run.step(false, start + Duration::from_secs(25)));
        assert_eq!(run.index, 0);
        run.finish(start + Duration::from_secs(26));
        let (shown, _, skipped) = run.summary();
        assert_eq!((shown, skipped), (2, 1));
    }

    #[test]
    fn replays_keep_order_times_or_filters() {
        let images = ids(4);
        let mut slides = deal(
            &images,
            &[
                Stage {
                    count: 2,
                    seconds: 30,
                },
                Stage {
                    count: 2,
                    seconds: 120,
                },
            ],
            Order::Board,
            &mut Rng::new(3),
        );
        for slide in &mut slides {
            slide.shown = true;
        }
        slides[1].skipped = true;
        slides[3].star = true;
        let gone = images[2];
        let exists = |id| id != gone;
        let same = replay(&slides, Replay::Same, exists, &mut Rng::new(1));
        assert_eq!(
            same.iter().map(|s| s.item).collect::<Vec<_>>(),
            vec![images[0], images[1], images[3]]
        );
        assert_eq!(same[2].seconds, 120);
        assert_eq!(
            replay(&slides, Replay::Skipped, exists, &mut Rng::new(1)).len(),
            1
        );
        let starred = replay(&slides, Replay::Starred, exists, &mut Rng::new(1));
        assert_eq!((starred.len(), starred[0].seconds), (1, 120));
        let shuffled = replay(&slides, Replay::Reshuffle, exists, &mut Rng::new(9));
        assert_eq!(
            shuffled.iter().map(|s| s.seconds).collect::<Vec<_>>(),
            vec![30, 30, 120]
        );
    }

    #[test]
    fn setup_roundtrips_and_rejects_bad_preferences() {
        let setup = Setup {
            interval: 120,
            plan: vec![
                Stage {
                    count: 10,
                    seconds: 30,
                },
                Stage {
                    count: 5,
                    seconds: 120,
                },
            ],
            order: Order::Random,
            mini: true,
            sound: false,
            ..Setup::default()
        };
        assert_eq!(Setup::decode(&setup.encode()), setup);
        assert_eq!(Setup::decode("junk"), Setup::default());
        assert_eq!(decode_plan("0x30"), None);
        assert_eq!(decode_plan("5x99999"), None);
        assert_eq!(setup.total(3), (15, Duration::from_secs(15 * 60)));
        assert_eq!(Setup::default().total(4), (4, Duration::from_secs(120)));
    }
}
