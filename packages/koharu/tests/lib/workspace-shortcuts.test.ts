import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { useKoharuStore } from '@/lib/store'
import { clampBarPosition, useTypeBarWorkspace } from '@/lib/typeBarWorkspace'
import { handleWorkspaceShortcut, useWorkspaceShortcuts } from '@/lib/workspaceShortcuts'

beforeEach(() => {
  useTypeBarWorkspace.setState({ position: null, hidden: false })
  useKoharuStore.getState().showPanel('layers')
  useKoharuStore.getState().selectLayers([])
})

const press = (key: string, options: KeyboardEventInit = {}) => {
  const event = new KeyboardEvent('keydown', { key, code: key, cancelable: true, ...options })
  handleWorkspaceShortcut(event)
  return event
}

describe('workspace shortcuts', () => {
  it('toggles Layers and Story with F7, including a physical-code-only event', () => {
    useKoharuStore.getState().selectLayers(['selected-text'])
    expect(press('F7').defaultPrevented).toBe(true)
    expect(useKoharuStore.getState().panel).toBe('story')
    expect(useKoharuStore.getState().selectedLayers).toEqual(['selected-text'])
    press('Unidentified', { code: 'F7' })
    expect(useKoharuStore.getState().panel).toBe('layers')
  })
  it('blurs an active Story field before switching panels', () => {
    useKoharuStore.getState().showPanel('story')
    const sidebar = document.createElement('aside')
    sidebar.dataset.workspaceSidebar = ''
    const field = document.createElement('textarea')
    sidebar.append(field)
    document.body.append(sidebar)
    try {
      let panelAtBlur: string | undefined
      field.addEventListener('blur', () => { panelAtBlur = useKoharuStore.getState().panel })
      field.focus()
      press('F7')
      expect(panelAtBlur).toBe('story')
      expect(useKoharuStore.getState().panel).toBe('layers')
    } finally { sidebar.remove() }
  })
  it('toggles visibility with F8 without changing the pinned position', () => {
    useTypeBarWorkspace.getState().setPosition({ x: 50, y: 80 })
    press('F8')
    expect(useTypeBarWorkspace.getState().hidden).toBe(true)
    press('F8', { repeat: true })
    expect(useTypeBarWorkspace.getState().hidden).toBe(true)
    press('F8')
    expect(useTypeBarWorkspace.getState().hidden).toBe(false)
    expect(useTypeBarWorkspace.getState().position).toEqual({ x: 50, y: 80 })
  })
  it('registers the handler while the workspace is mounted and removes it on unmount', () => {
    const hook = renderHook(() => useWorkspaceShortcuts())
    act(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'F7', code: 'F7', cancelable: true })) })
    expect(useKoharuStore.getState().panel).toBe('story')
    hook.unmount()
    act(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'F7', code: 'F7', cancelable: true })) })
    expect(useKoharuStore.getState().panel).toBe('story')
  })
  it('does not intercept composing or modified keys', () => {
    expect(press('F7', { isComposing: true }).defaultPrevented).toBe(false)
    expect(press('F8', { ctrlKey: true }).defaultPrevented).toBe(false)
    expect(useKoharuStore.getState().panel).toBe('layers')
    expect(useTypeBarWorkspace.getState().hidden).toBe(false)
  })
  it('clamps a pinned bar inside the viewport', () => {
    expect(clampBarPosition({ x: 1000, y: -40 }, { width: 500, height: 400 }, { width: 200, height: 50 })).toEqual({ x: 292, y: 8 })
    expect(clampBarPosition({ x: 50, y: 50 }, { width: 100, height: 40 }, { width: 200, height: 50 })).toEqual({ x: 0, y: 0 })
  })
})

