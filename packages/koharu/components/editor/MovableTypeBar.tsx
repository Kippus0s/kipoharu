'use client'

import { EyeOff, GripVertical, Pin, PinOff } from 'lucide-react'
import { useRef, useState, type ComponentProps, type PointerEvent, type KeyboardEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { ContextBar } from '@/components/editor/ContextBar'
import { clampBarPosition, useTypeBarWorkspace, type BarPosition } from '@/lib/typeBarWorkspace'

function locate(control: HTMLElement) {
  const bar = control.closest<HTMLElement>('[data-testid="type-bar"]')
  const parent = bar?.parentElement
  if (!bar || !parent) return null
  const viewport = parent.getBoundingClientRect()
  const rect = bar.getBoundingClientRect()
  return { viewport, rect, position: { x: rect.left - viewport.left, y: rect.top - viewport.top } }
}

const buttonClass = 'grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-foreground/[0.08] hover:text-foreground'

/** Only the grip moves the bar. Other controls keep their ordinary pointer behaviour. */
export function MovableTypeBar(props: ComponentProps<typeof ContextBar>) {
  const hidden = useTypeBarWorkspace((state) => state.hidden)
  // TypeBar itself stays mounted: its font-cycle shortcut still works while hidden.
  return hidden ? null : <VisibleTypeBar {...props} />
}

function VisibleTypeBar({ children, ...props }: ComponentProps<typeof ContextBar>) {
  const { t } = useTranslation()
  const position = useTypeBarWorkspace((state) => state.position)
  const setPosition = useTypeBarWorkspace((state) => state.setPosition)
  const setHidden = useTypeBarWorkspace((state) => state.setHidden)
  const [preview, setPreview] = useState<BarPosition | null>(null)
  const drag = useRef<{ id: number; x: number; y: number; start: BarPosition; moved: boolean } | null>(null)

  const down = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || !event.isPrimary || drag.current) return
    const geometry = locate(event.currentTarget)
    if (!geometry) return
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.setPointerCapture(event.pointerId)
    drag.current = { id: event.pointerId, x: event.clientX, y: event.clientY, start: geometry.position, moved: false }
  }
  const nextPosition = (event: PointerEvent<HTMLButtonElement>) => {
    const active = drag.current
    if (!active || active.id !== event.pointerId) return null
    const dx = event.clientX - active.x
    const dy = event.clientY - active.y
    if (!active.moved && Math.hypot(dx, dy) < 3) return null
    active.moved = true
    const geometry = locate(event.currentTarget)
    return geometry ? clampBarPosition({ x: active.start.x + dx, y: active.start.y + dy }, geometry.viewport, geometry.rect) : null
  }
  const finish = (event: PointerEvent<HTMLButtonElement>, commit: boolean) => {
    if (drag.current?.id !== event.pointerId) return
    const next = commit ? nextPosition(event) : null
    drag.current = null
    setPreview(null)
    if (next) setPosition(next)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }
  const moveWithKeys = (event: KeyboardEvent<HTMLButtonElement>) => {
    const directions: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }
    const delta = directions[event.key]
    const geometry = locate(event.currentTarget)
    if (!delta || !geometry) return
    event.preventDefault()
    event.stopPropagation()
    const step = event.shiftKey ? 32 : 8
    setPosition(clampBarPosition({ x: geometry.position.x + delta[0] * step, y: geometry.position.y + delta[1] * step }, geometry.viewport, geometry.rect))
  }

  return (
    <ContextBar {...props} pinnedPosition={preview ?? position}>
      <div className='flex shrink-0 items-center gap-0.5 border-r border-border pr-1'>
        <button type='button' className={buttonClass + ' touch-none cursor-grab select-none active:cursor-grabbing'}
          title={t('typeBarWorkspace.drag')} aria-label={t('typeBarWorkspace.drag')}
          onPointerDown={down} onPointerMove={(event) => { const next = nextPosition(event); if (next) setPreview(next) }}
          onPointerUp={(event) => finish(event, true)} onPointerCancel={(event) => finish(event, false)}
          onLostPointerCapture={(event) => finish(event, false)} onKeyDown={moveWithKeys}>
          <GripVertical className='size-4' />
        </button>
        <button type='button' className={buttonClass + (position ? ' text-primary' : '')}
          title={t(position ? 'typeBarWorkspace.unpin' : 'typeBarWorkspace.pin')}
          aria-label={t(position ? 'typeBarWorkspace.unpin' : 'typeBarWorkspace.pin')}
          onClick={(event) => {
            if (position) setPosition(null)
            else { const geometry = locate(event.currentTarget); if (geometry) setPosition(geometry.position) }
          }}>
          {position ? <PinOff className='size-3.5' /> : <Pin className='size-3.5' />}
        </button>
        <button type='button' className={buttonClass} title={t('typeBarWorkspace.hide')} aria-label={t('typeBarWorkspace.hide')} onClick={() => setHidden(true)}>
          <EyeOff className='size-3.5' />
        </button>
      </div>
      {children}
    </ContextBar>
  )
}

