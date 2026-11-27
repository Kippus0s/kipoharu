'use client'

import { useEffect } from 'react'

import { shortcuts } from './shortcuts'
import { useKoharuStore } from './store'
import { useTypeBarWorkspace } from './typeBarWorkspace'

/** Capture function keys before the webview's defaults and other hotkey handlers. */
export function handleWorkspaceShortcut(event: KeyboardEvent) {
  if (
    event.isComposing ||
    event.key === 'Process' ||
    event.key === 'Dead' ||
    event.keyCode === 229 ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey ||
    event.shiftKey
  )
    return
  const matches = (shortcut: string, legacyCode: number) =>
    event.code.toLowerCase() === shortcut ||
    event.key.toLowerCase() === shortcut ||
    ((!event.key || event.key === 'Unidentified') && event.keyCode === legacyCode)
  const layers = matches(shortcuts.layers, 118)
  const typeBar = matches(shortcuts.typeBar, 119)
  if (!layers && !typeBar) return
  event.preventDefault()
  event.stopImmediatePropagation()
  if (event.repeat) return
  const active = document.activeElement
  if (layers) {
    // Blurring a Story field lets CommitTextarea flush its draft before the tab is hidden.
    if (
      active instanceof HTMLElement &&
      active.closest('[data-workspace-sidebar]') &&
      active.matches('textarea, input, [contenteditable="true"]')
    )
      active.blur()
    const store = useKoharuStore.getState()
    store.showPanel(store.panel === 'layers' ? 'story' : 'layers')
  } else {
    const store = useTypeBarWorkspace.getState()
    if (
      !store.hidden &&
      active instanceof HTMLElement &&
      active.closest('[data-testid="type-bar"]')
    )
      active.blur()
    store.setHidden(!store.hidden)
  }
}

export function useWorkspaceShortcuts() {
  useEffect(() => {
    window.addEventListener('keydown', handleWorkspaceShortcut, true)
    return () => window.removeEventListener('keydown', handleWorkspaceShortcut, true)
  }, [])
}
