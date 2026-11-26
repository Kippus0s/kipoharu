import { describe, expect, it } from 'vitest'

import { dropMoves, lockTargets } from '@/lib/document'
import type { Layer } from '@koharu/ipc/protocol'

/** A shown, unlocked layer of `kind` holding `children`, with only what dropping reads. */
function layer(id: string, kind: Layer['kind'], children: Layer[] = []): Layer {
  return { id, kind, visible: true, locked: false, opacity: 1, children } as unknown as Layer
}

describe('dropMoves', () => {
  it('counts each move after the ones before it, so dragged layers keep their order', () => {
    const page = ['a', 'b', 'c', 'd'].map((id) => layer(id, 'raster'))
    expect(dropMoves(page, ['a', 'b'], { target: 'd', zone: 'after' })).toEqual([
      { layer: 'a', parent: null, index: 3 },
      { layer: 'b', parent: null, index: 3 },
    ])
    expect(dropMoves(page, ['a'], { target: 'b', zone: 'before' })).toEqual([])
  })

  it('puts a layer only where the scene lets it sit', () => {
    const page = [
      layer('panel', 'panel', [layer('balloon', 'balloon', [layer('text', 'block')])]),
      layer('raster', 'raster'),
      layer('free', 'block'),
    ]
    expect(dropMoves(page, ['raster'], { target: 'balloon', zone: 'inside' })).toBeNull()
    expect(dropMoves(page, ['free'], { target: 'text', zone: 'inside' })).toBeNull()
    expect(dropMoves(page, ['free'], { target: 'balloon', zone: 'inside' })).toEqual([
      { layer: 'free', parent: 'balloon', index: 1 },
    ])
    expect(dropMoves(page, ['panel'], { target: 'balloon', zone: 'after' })).toBeNull()
  })
})

describe('lockTargets', () => {
  it('locks children before their selected parents and unlocks in the opposite order', () => {
    const child = layer('child', 'text')
    const parent = layer('parent', 'group', [child])
    const separate = layer('separate', 'text')
    const selected = ['parent', 'child', 'separate']
    expect(lockTargets([parent, separate], selected, true)).toEqual(['separate', 'child', 'parent'])
    parent.locked = child.locked = separate.locked = true
    expect(lockTargets([parent, separate], selected, false)).toEqual(['parent', 'child', 'separate'])
  })

  it('skips unchanged layers and children of unselected locked parents', () => {
    const child = layer('child', 'text')
    const parent = layer('parent', 'group', [child])
    const separate = layer('separate', 'text')
    parent.locked = true
    expect(lockTargets([parent, separate], ['child', 'separate'], true)).toEqual(['separate'])
    child.locked = true
    expect(lockTargets([parent, separate], ['child', 'separate'], false)).toEqual([])
  })
})
