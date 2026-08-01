import { fuzzySegments } from "./fuzzy"
export function FuzzyText({ text, indices }: { text: string; indices: ReadonlyArray<number> }) {
  const segments = fuzzySegments(text, indices)
  return (
    <>
      {segments.map((segment, i) =>
        segment.match ? (
          <strong key={i} className="text-foreground font-semibold">
            {segment.text}
          </strong>
        ) : (
          <span key={i}>{segment.text}</span>
        ),
      )}
    </>
  )
}
