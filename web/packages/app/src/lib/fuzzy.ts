export interface FuzzyMatch {
  readonly score: number
  readonly indices: ReadonlyArray<number>
}

// Subsequence match: every character of `query` must appear in `text` in order. A lower score sorts
// first — earlier first hit and tighter (more contiguous) runs win. An empty query matches anything
// with no highlight, so a freshly focused field shows the whole list.
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  if (query === "") return { score: 0, indices: [] }
  const q = query.toLowerCase()
  const t = text.toLowerCase()
  const indices: number[] = []
  let qi = 0
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] === q[qi]) {
      indices.push(i)
      qi++
    }
  }
  if (qi < q.length) return null
  let score = indices[0] * 1.5
  for (let k = 1; k < indices.length; k++) score += indices[k] - indices[k - 1] - 1
  return { score, indices }
}

export interface FuzzySegment {
  readonly text: string
  readonly match: boolean
}

// Collapse matched indices into contiguous runs so a renderer can emphasize whole segments rather
// than one span per character.
export function fuzzySegments(text: string, indices: ReadonlyArray<number>): FuzzySegment[] {
  if (indices.length === 0) return text === "" ? [] : [{ text, match: false }]
  const matched = new Set(indices)
  const segments: FuzzySegment[] = []
  let start = 0
  let current = matched.has(0)
  for (let i = 1; i <= text.length; i++) {
    const isMatch = i < text.length && matched.has(i)
    if (i === text.length || isMatch !== current) {
      segments.push({ text: text.slice(start, i), match: current })
      start = i
      current = isMatch
    }
  }
  return segments
}
