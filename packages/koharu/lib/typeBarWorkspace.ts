'use client'

import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type BarPosition = { x: number; y: number }
interface Workspace {
  position: BarPosition | null
  hidden: boolean
  setPosition: (position: BarPosition | null) => void
  setHidden: (hidden: boolean) => void
}

export const useTypeBarWorkspace = create<Workspace>()(
  persist(
    (set) => ({
      position: null,
      hidden: false,
      setPosition: (position) => set({ position }),
      setHidden: (hidden) => set({ hidden }),
    }),
    {
      name: 'kipoharu-typebar-workspace',
      partialize: ({ position, hidden }) => ({ position, hidden }),
    },
  ),
)

/** Keep the whole bar in reach, even after the canvas is resized. */
export function clampBarPosition(
  position: BarPosition,
  viewport: { width: number; height: number },
  bar: { width: number; height: number },
): BarPosition {
  const axis = (value: number, outer: number, inner: number) => {
    const edge = Math.min(8, Math.max(0, (outer - inner) / 2))
    return Math.max(
      edge,
      Math.min(Number.isFinite(value) ? value : edge, Math.max(edge, outer - inner - edge)),
    )
  }
  return {
    x: axis(position.x, viewport.width, bar.width),
    y: axis(position.y, viewport.height, bar.height),
  }
}
