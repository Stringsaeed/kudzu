// Group tree helpers (nesting, ordering, effective enablement).

import type { Group, Snippet } from "./types"

export interface GroupNode {
  group: Group
  depth: number
  children: GroupNode[]
}

export function buildTree(groups: Group[]): GroupNode[] {
  const byParent = new Map<string | undefined, Group[]>()
  const ids = new Set(groups.map((g) => g.id))
  for (const g of groups) {
    const parent = g.parentID && ids.has(g.parentID) && g.parentID !== g.id ? g.parentID : undefined
    if (!byParent.has(parent)) byParent.set(parent, [])
    byParent.get(parent)!.push(g)
  }
  const seen = new Set<string>()
  const walk = (parent: string | undefined, depth: number): GroupNode[] =>
    (byParent.get(parent) ?? [])
      .sort((a, b) => a.sortIndex - b.sortIndex || a.name.localeCompare(b.name))
      .filter((g) => !seen.has(g.id) && seen.add(g.id))
      .map((g) => ({ group: g, depth, children: walk(g.id, depth + 1) }))
  return walk(undefined, 0)
}

export function flatten(nodes: GroupNode[], collapsed?: Set<string>): GroupNode[] {
  const out: GroupNode[] = []
  const visit = (list: GroupNode[]) => {
    for (const n of list) {
      out.push(n)
      if (!collapsed?.has(n.group.id)) visit(n.children)
    }
  }
  visit(nodes)
  return out
}

export function descendantIds(groups: Group[], id: string): Set<string> {
  const out = new Set([id])
  let grew = true
  while (grew) {
    grew = false
    for (const g of groups) {
      if (g.parentID && out.has(g.parentID) && !out.has(g.id)) {
        out.add(g.id)
        grew = true
      }
    }
  }
  return out
}

export function groupPath(groups: Group[], id: string | undefined): string[] {
  const byId = new Map(groups.map((g) => [g.id, g]))
  const path: string[] = []
  const seen = new Set<string>()
  let cur = id ? byId.get(id) : undefined
  while (cur && !seen.has(cur.id)) {
    seen.add(cur.id)
    path.unshift(cur.name)
    cur = cur.parentID ? byId.get(cur.parentID) : undefined
  }
  return path
}

export function isGroupEnabled(groups: Group[], id: string | undefined): boolean {
  const byId = new Map(groups.map((g) => [g.id, g]))
  const seen = new Set<string>()
  let cur = id ? byId.get(id) : undefined
  while (cur && !seen.has(cur.id)) {
    if (!cur.isEnabled) return false
    seen.add(cur.id)
    cur = cur.parentID ? byId.get(cur.parentID) : undefined
  }
  return true
}

export function recursiveCounts(groups: Group[], snippets: Snippet[]): Map<string, number> {
  const parent = new Map(groups.map((g) => [g.id, g.parentID]))
  const counts = new Map<string, number>()
  for (const s of snippets) {
    const seen = new Set<string>()
    let g = s.groupID
    while (g && parent.has(g) && !seen.has(g)) {
      seen.add(g)
      counts.set(g, (counts.get(g) ?? 0) + 1)
      g = parent.get(g)
    }
  }
  return counts
}
