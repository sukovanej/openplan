// Pinned to English rather than the viewer's locale: the rest of the UI is English-only, so a
// localized date reads as a translation gap instead of a feature.
const RELATIVE = new Intl.RelativeTimeFormat("en", { numeric: "always" })
const ABSOLUTE = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" })

// Largest unit first: the span is reported in the coarsest unit it fills, so an age reads "3 days
// ago" rather than "72 hours ago".
const UNITS: ReadonlyArray<[Intl.RelativeTimeFormatUnit, number]> = [
  ["year", 365 * 24 * 60 * 60],
  ["month", 30 * 24 * 60 * 60],
  ["week", 7 * 24 * 60 * 60],
  ["day", 24 * 60 * 60],
  ["hour", 60 * 60],
  ["minute", 60],
]

export function relativeTime(iso: string, now: number = Date.now()): string {
  const at = Date.parse(iso)
  if (Number.isNaN(at)) return iso
  const seconds = (at - now) / 1000
  for (const [unit, size] of UNITS) {
    const count = seconds / size
    // Truncated, not rounded: the unit is picked from the whole part, so rounding it back up would
    // print the next unit's value in this one's name — "24 hours ago", "60 minutes ago".
    if (Math.abs(count) >= 1) return RELATIVE.format(Math.trunc(count), unit)
  }
  return "just now"
}

export function absoluteTime(iso: string): string {
  const at = Date.parse(iso)
  return Number.isNaN(at) ? iso : ABSOLUTE.format(at)
}
