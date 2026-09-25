// A Map keeps the order of insertion, so a hit moves its entry to the end, and an entry past the limit
// leaves from the front: the one used longest ago.
export class BoundedCache<K, V> {
  private readonly entries = new Map<K, V>()

  constructor(private readonly limit: number) {}

  get size(): number {
    return this.entries.size
  }

  has(key: K): boolean {
    return this.entries.has(key)
  }

  remember(key: K, make: () => V): V {
    if (this.entries.has(key)) {
      const held = this.entries.get(key) as V
      this.entries.delete(key)
      this.entries.set(key, held)
      return held
    }
    const made = make()
    this.entries.set(key, made)
    for (const oldest of this.entries.keys()) {
      if (this.entries.size <= this.limit) break
      this.entries.delete(oldest)
    }
    return made
  }
}
