/*!
Defines data types shared between `jiff` and `jiff-static`.

While this module exposes types that can be imported outside of `jiff` itself,
there are *no* semver guarantees provided. That is, this module is _not_ part
of Jiff's public API. The only guarantee of compatibility that is provided
is that `jiff-static x.y.z` works with one and only one version of Jiff,
corresponding to `jiff x.y.z` (i.e., the same version number).

# Design

This module is really accomplishing two different things at the same time.

Firstly, it is a way to provide types that can be used to construct a static
`TimeZone`. The proc macros in `jiff-static` generate code using these
types (and a few routines).

Secondly, it provides a way to parse TZif data without `jiff-static`
depending on `jiff` via a Cargo dependency. This actually requires copying
the code in this module (which is why it is kinda sectioned off from the rest
of jiff) into the `jiff-static` crate. This can be done automatically with
`jiff-cli`:

```text
jiff-cli generate shared
```

The copying of code is pretty unfortunate, because it means both crates have to
compile it. However, the alternatives aren't great either.

One alternative is to have `jiff-static` explicitly depend on `jiff` in its
`Cargo.toml`. Then Jiff could expose the parsing routines, as it does here,
and `jiff-static` could use them directly. Unfortunately, this means that
`jiff` cannot depend on `jiff-static`. And that in turn means that `jiff`
cannot re-export the macros. Users will need to explicitly depend on and use
`jiff-static`. Moreover, this could result in some potential surprises
since `jiff-static` will need to have an `=x.y.z` dependency on Jiff for
compatibility reasons. That in turn means that the version of Jiff actually
used is not determine by the user's `jiff = "x.y.z"` line, but rather by the
user's `jiff-static = "x'.y'.z'"` line. This is overall annoying and not a
good user experience. Plus, it inverts the typical relationship between crates
and their proc macros (e.g., `serde` and `serde_derive`) and thus could result
in other unanticipated surprises.

Another obvious alternative is to split this code out into a separate crate
that both `jiff` and `jiff-static` depend on. However, the API exposed in
this module does not provide a coherent user experience. It would either need a
ton of work to turn it into a coherent user experience or it would need to be
published as a `jiff-internal-use-only` crate that I find to be very annoying
and confusing. Moreover, a separate crate introduces a new semver boundary
beneath Jiff. I've found these sorts of things to overall increase maintenance
burden (see ripgrep and regex for cases where I did this).

I overall decided that the least bad choice was to copy a little code (under
2,000 source lines of code at present I believe). Since the copy is managed
automatically via `jiff-cli generate shared`, we remove the downside of the
code getting out of sync. The only downside is extra compile time. Since I
generally only expect `jiff-static` to be used in niche circumstances, I
prefer this trade-off over the other choices.

More context on how I arrived at this design can be found here:
<https://github.com/BurntSushi/jiff/issues/256>

# Particulars

When this code is copied to `jiff-static`, the following transformations are
done:

* A header is added to indicate that the copied file is auto-generated.
* All `#[cfg(feature = "alloc")]` annotations are removed. The `jiff-static`
  proc macro always runs in a context where the standard library is available.
* Any code between `// only-jiff-start` and `// only-jiff-end` comments is
  removed. Nesting isn't supported.

Otherwise, this module is specifically organized in a way that doesn't rely on
any other part of Jiff. The one exception are routines to convert from these
exposed types to other internal types inside of Jiff. This is necessary for
building a static `TimeZone`. But these conversion routines are removed when
this module is copied to `jiff-static`.
*/

// only-jiff-start
pub type TzifStatic = Tzif<
    &'static str,
    &'static str,
    &'static [TzifLocalTimeType],
    &'static [TzifTransition],
>;
// only-jiff-end

#[cfg(feature = "alloc")]
pub type TzifOwned = Tzif<
    alloc::string::String,
    self::util::array_str::Abbreviation,
    alloc::vec::Vec<TzifLocalTimeType>,
    alloc::vec::Vec<TzifTransition>,
>;

#[derive(Clone, Debug)]
pub struct Tzif<STRING, ABBREV, TYPES, TRANS> {
    pub fixed: TzifFixed<STRING, ABBREV>,
    pub types: TYPES,
    pub transitions: TRANS,
}

#[derive(Clone, Debug)]
pub struct TzifFixed<STRING, ABBREV> {
    pub name: Option<STRING>,
    pub version: u8,
    pub checksum: u32,
    pub designations: STRING,
    pub posix_tz: Option<PosixTimeZone<ABBREV>>,
}

// only-jiff-start
impl TzifFixed<&'static str, &'static str> {
    pub const fn into_jiff(
        self,
        types: &'static [crate::tz::tzif::LocalTimeType],
        trans: &'static [crate::tz::tzif::Transition],
    ) -> crate::tz::tzif::TzifStatic {
        crate::tz::tzif::TzifStatic::from_shared_const(self, types, trans)
    }
}
// only-jiff-end

#[derive(Clone, Debug)]
pub struct TzifLocalTimeType {
    pub offset: i32,
    pub is_dst: bool,
    pub designation: (u8, u8), // inclusive..exclusive
    pub indicator: TzifIndicator,
}

// only-jiff-start
impl TzifLocalTimeType {
    pub const fn into_jiff(self) -> crate::tz::tzif::LocalTimeType {
        crate::tz::tzif::LocalTimeType::from_shared(self)
    }
}
// only-jiff-end

#[derive(Clone, Debug)]
pub enum TzifIndicator {
    LocalWall,
    LocalStandard,
    UTStandard,
}

#[derive(Clone, Debug)]
pub struct TzifTransitions<TIMESTAMPS, STARTS, ENDS, INFOS> {
    pub timestamps: TIMESTAMPS,
    pub civil_starts: STARTS,
    pub civil_ends: ENDS,
    pub infos: INFOS,
}

#[derive(Clone, Debug)]
pub struct TzifTransitionInfo {
    pub type_index: u8,
    pub kind: TzifTransitionKind,
}

/// The kind of a transition.
///
/// This is used when trying to determine the offset for a local datetime. It
/// indicates how the corresponding civil datetimes in `civil_starts` and
/// `civil_ends` should be interpreted. That is, there are three possible
/// cases:
///
/// 1. The offset of this transition is equivalent to the offset of the
/// previous transition. That means there are no ambiguous civil datetimes
/// between the transitions. This can occur, e.g., when the time zone
/// abbreviation changes.
/// 2. The offset of the transition is greater than the offset of the previous
/// transition. That means there is a "gap" in local time between the
/// transitions. This typically corresponds to entering daylight saving time.
/// It is usually, but not always, 1 hour.
/// 3. The offset of the transition is less than the offset of the previous
/// transition. That means there is a "fold" in local time where time is
/// repeated. This typically corresponds to leaving daylight saving time. It
/// is usually, but not always, 1 hour.
#[derive(Clone, Debug)]
pub enum TzifTransitionKind {
    /// This transition cannot possibly lead to an unambiguous offset because
    /// its offset is equivalent to the offset of the previous transition.
    ///
    /// Has an entry in `civil_starts`, but corresponding entry in `civil_ends`
    /// is always zeroes (i.e., meaningless).
    Unambiguous,
    /// This occurs when this transition's offset is strictly greater than the
    /// previous transition's offset. This effectively results in a "gap" of
    /// time equal to the difference in the offsets between the two
    /// transitions.
    ///
    /// Has an entry in `civil_starts` for when the gap starts (inclusive) in
    /// local time. Also has an entry in `civil_ends` for when the fold ends
    /// (exclusive) in local time.
    Gap,
    /// This occurs when this transition's offset is strictly less than the
    /// previous transition's offset. This results in a "fold" of time where
    /// the two transitions have an overlap where it is ambiguous which one
    /// applies given a wall clock time. In effect, a span of time equal to the
    /// difference in the offsets is repeated.
    ///
    /// Has an entry in `civil_starts` for when the fold starts (inclusive) in
    /// local time. Also has an entry in `civil_ends` for when the fold ends
    /// (exclusive) in local time.
    Fold,
}

#[derive(Clone, Debug)]
pub struct TzifTransition {
    pub timestamp: i64,
    pub type_index: u8,
}

// only-jiff-start
impl TzifTransition {
    pub const fn into_jiff(
        self,
        prev_offset: i32,
        this_offset: i32,
    ) -> crate::tz::tzif::Transition {
        crate::tz::tzif::Transition::from_shared(
            self,
            prev_offset,
            this_offset,
        )
    }
}
// only-jiff-end

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixTimeZone<ABBREV> {
    pub std_abbrev: ABBREV,
    pub std_offset: PosixOffset,
    pub dst: Option<PosixDst<ABBREV>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixDst<ABBREV> {
    pub abbrev: ABBREV,
    pub offset: PosixOffset,
    pub rule: PosixRule,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixRule {
    pub start: PosixDayTime,
    pub end: PosixDayTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixDayTime {
    pub date: PosixDay,
    pub time: PosixTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PosixDay {
    /// Julian day in a year, no counting for leap days.
    ///
    /// Valid range is `1..=365`.
    JulianOne(i16),
    /// Julian day in a year, counting for leap days.
    ///
    /// Valid range is `0..=365`.
    JulianZero(i16),
    /// The nth weekday of a month.
    WeekdayOfMonth {
        /// The month.
        ///
        /// Valid range is: `1..=12`.
        month: i8,
        /// The week.
        ///
        /// Valid range is `1..=5`.
        ///
        /// One interesting thing to note here (or my interpretation anyway),
        /// is that a week of `4` means the "4th weekday in a month" where as
        /// a week of `5` means the "last weekday in a month, even if it's the
        /// 4th weekday."
        week: i8,
        /// The weekday.
        ///
        /// Valid range is `0..=6`, with `0` corresponding to Sunday.
        weekday: i8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixTime {
    pub second: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosixOffset {
    pub second: i32,
}

// only-jiff-start
impl PosixTimeZone<&'static str> {
    pub const fn into_jiff(self) -> crate::tz::posix::PosixTimeZoneStatic {
        crate::tz::posix::PosixTimeZone::from_shared_const(self)
    }
}
// only-jiff-end

// Does not require `alloc`, but is only used when `alloc` is enabled.
#[cfg(feature = "alloc")]
pub(crate) mod crc32;
pub(crate) mod posix;
#[cfg(feature = "alloc")]
pub(crate) mod tzif;
pub(crate) mod util;
