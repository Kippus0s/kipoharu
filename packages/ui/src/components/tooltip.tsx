'use client'

import { Tooltip as TooltipPrimitive } from '@base-ui/react/tooltip'
import { createContext, useContext, useEffect, useRef, useState } from 'react'

import { cn } from '@koharu/ui/lib/utils'

const RearmTooltip = createContext<((trigger: HTMLElement) => void) | undefined>(undefined)

function TooltipProvider({ delay = 350, ...props }: TooltipPrimitive.Provider.Props) {
  return <TooltipPrimitive.Provider data-slot='tooltip-provider' delay={delay} closeDelay={100} {...props} />
}

function Tooltip({ defaultOpen, open: controlledOpen, onOpenChange, ...props }: TooltipPrimitive.Root.Props) {
  const [open, setOpen] = useState(defaultOpen ?? false)
  const blocked = useRef(false)
  const trigger = useRef<HTMLElement | null>(null)
  // All current tooltips are uncontrolled by their callers. Preserve Base UI's
  // controlled API for callers that explicitly own the open state.
  const visible = controlledOpen ?? open
  useEffect(() => {
    if (!visible || controlledOpen !== undefined) return
    const dismiss = () => {
      blocked.current = true
      setOpen(false)
    }
    const timer = window.setTimeout(dismiss, 3000)
    const moved = (event: PointerEvent) => {
      if (trigger.current && event.target instanceof Node && !trigger.current.contains(event.target)) {
        dismiss()
      }
    }
    window.addEventListener('pointermove', moved, true)
    window.addEventListener('pointerdown', dismiss, true)
    window.addEventListener('blur', dismiss)
    return () => {
      window.clearTimeout(timer)
      window.removeEventListener('pointermove', moved, true)
      window.removeEventListener('pointerdown', dismiss, true)
      window.removeEventListener('blur', dismiss)
    }
  }, [visible, controlledOpen])
  return (
    <RearmTooltip.Provider value={(element) => {
      trigger.current = element
      blocked.current = false
    }}>
      <TooltipPrimitive.Root
        data-slot='tooltip'
        {...props}
        disableHoverablePopup
        open={visible}
        onOpenChange={(next, details) => {
          if (next && blocked.current && controlledOpen === undefined) {
            details.cancel()
            return
          }
          onOpenChange?.(next, details)
          if (!details.isCanceled) setOpen(next)
        }}
      />
    </RearmTooltip.Provider>
  )
}

function TooltipTrigger({ onPointerEnter, onFocus, ...props }: TooltipPrimitive.Trigger.Props) {
  const rearm = useContext(RearmTooltip)
  return (
    <TooltipPrimitive.Trigger
      data-slot='tooltip-trigger'
      {...props}
      onPointerEnter={(event) => {
        rearm?.(event.currentTarget)
        onPointerEnter?.(event)
      }}
      onFocus={(event) => {
        // Clicking an already-hovered control must not rearm an expired tip.
        if (event.currentTarget.matches(':focus-visible')) rearm?.(event.currentTarget)
        onFocus?.(event)
      }}
    />
  )
}

function TooltipContent({
  className,
  side = 'top',
  sideOffset = 4,
  align = 'center',
  alignOffset = 0,
  children,
  ...props
}: TooltipPrimitive.Popup.Props &
  Pick<TooltipPrimitive.Positioner.Props, 'align' | 'alignOffset' | 'side' | 'sideOffset'>) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Positioner
        align={align}
        alignOffset={alignOffset}
        side={side}
        sideOffset={sideOffset}
        className='pointer-events-none isolate z-50'
      >
        <TooltipPrimitive.Popup
          data-slot='tooltip-content'
          className={cn(
            'pointer-events-none z-50 inline-flex w-fit max-w-xs origin-(--transform-origin) items-center gap-1.5 rounded-md bg-foreground px-3 py-1.5 text-xs text-background has-data-[slot=kbd]:pr-1.5 data-[side=bottom]:slide-in-from-top-2 data-[side=inline-end]:slide-in-from-left-2 data-[side=inline-start]:slide-in-from-right-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 **:data-[slot=kbd]:relative **:data-[slot=kbd]:isolate **:data-[slot=kbd]:z-50 **:data-[slot=kbd]:rounded-sm data-[state=delayed-open]:animate-in data-[state=delayed-open]:fade-in-0 data-[state=delayed-open]:zoom-in-95 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95',
            className,
          )}
          {...props}
        >
          {children}
          <TooltipPrimitive.Arrow className='z-50 size-2.5 translate-y-[calc(-50%-2px)] rotate-45 rounded-[2px] bg-foreground fill-foreground data-[side=bottom]:top-1 data-[side=inline-end]:top-1/2! data-[side=inline-end]:-left-1 data-[side=inline-end]:-translate-y-1/2 data-[side=inline-start]:top-1/2! data-[side=inline-start]:-right-1 data-[side=inline-start]:-translate-y-1/2 data-[side=left]:top-1/2! data-[side=left]:-right-1 data-[side=left]:-translate-y-1/2 data-[side=right]:top-1/2! data-[side=right]:-left-1 data-[side=right]:-translate-y-1/2 data-[side=top]:-bottom-2.5' />
        </TooltipPrimitive.Popup>
      </TooltipPrimitive.Positioner>
    </TooltipPrimitive.Portal>
  )
}

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider }
