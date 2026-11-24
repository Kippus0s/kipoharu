'use client'

import { autoUpdate, flip, offset, shift, useFloating } from '@floating-ui/react-dom'
import { useLayoutEffect, useState, type ComponentProps } from 'react'

import { useKoharuStore } from '@/lib/store'
import { clampBarPosition, type BarPosition } from '@/lib/typeBarWorkspace'
import type { Bounds } from '@koharu/ipc/protocol'
import { cn } from '@koharu/ui/lib/utils'

/** The space between the bar and what it's beside, and the canvas's edges, in CSS pixels. */
const GAP = 8

/** A bar beside a selection on the canvas, as Photoshop's Contextual Task Bar sits by one: centred
 * below `bounds`, in page pixels the camera places, or above when there's no room below, kept
 * inside the canvas, which clips it. */
export function ContextBar({
  bounds,
  className,
  style,
  pinnedPosition,
  ...props
}: { bounds: Bounds; pinnedPosition?: BarPosition | null } & ComponentProps<'div'>) {
  const camera = useKoharuStore((state) => state.camera)
  const { refs, floatingStyles, isPositioned } = useFloating({
    placement: 'bottom',
    middleware: [offset(GAP), flip({ padding: GAP }), shift({ padding: GAP, crossAxis: true })],
    whileElementsMounted: autoUpdate,
  })

  const pinned = pinnedPosition != null
  const [dimensions, setDimensions] = useState({ viewport: { width: 0, height: 0 }, bar: { width: 0, height: 0 } })
  useLayoutEffect(() => {
    const element = refs.floating.current
    const parent = element?.parentElement
    if (!pinned || !element || !parent) return
    const measure = () => {
      const viewport = { width: parent.clientWidth, height: parent.clientHeight }
      const bar = { width: element.offsetWidth, height: element.offsetHeight }
      setDimensions((previous) =>
        previous.viewport.width === viewport.width && previous.viewport.height === viewport.height &&
        previous.bar.width === bar.width && previous.bar.height === bar.height
          ? previous : { viewport, bar },
      )
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(parent)
    observer.observe(element)
    return () => observer.disconnect()
  }, [pinned, refs])
  const fixed = pinnedPosition == null ? null : clampBarPosition(pinnedPosition, dimensions.viewport, dimensions.bar)

  // The selection as the canvas element shows it, the bar's parent.
  useLayoutEffect(() => {
    refs.setReference({
      getBoundingClientRect: () => {
        const canvas = refs.floating.current?.parentElement?.getBoundingClientRect()
        return new DOMRect(
          (canvas?.left ?? 0) + bounds.x * camera.zoom + camera.x,
          (canvas?.top ?? 0) + bounds.y * camera.zoom + camera.y,
          bounds.width * camera.zoom,
          bounds.height * camera.zoom,
        )
      },
    })
  }, [bounds, camera, refs])

  return (
    <div
      ref={refs.setFloating}
      role='toolbar'
      className={cn(
        'z-20 flex max-w-[calc(100%-16px)] items-center gap-1 rounded-xl border border-border bg-[var(--surface-floating)] p-1.5 shadow-[var(--shadow-toolrail)]',
        className,
      )}
      style={{
        ...style,
        ...(fixed ? { position: 'absolute' as const, left: fixed.x, top: fixed.y, transform: 'none' } : floatingStyles),
        visibility: fixed || isPositioned ? 'visible' : 'hidden',
      }}
      {...props}
    />
  )
}
