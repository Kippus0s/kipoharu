import { act, render, screen, waitFor } from '@testing-library/react'
import { createElement, StrictMode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import Providers from '@/app/providers'
import { projectKey, queryClient, readPage, usePage, usePageStory, useProject } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import { commands, events, type Change, type Preferences } from '@koharu/ipc/protocol'

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {},
  invoke: vi.fn(),
}))

/** Each event's handlers, as the interface listened. */
const handlers = new Map<string, ((payload: unknown) => void)[]>()

/** Tells `payload` as the desktop would. */
function tell(name: keyof typeof events, payload: unknown): void {
  for (const handler of handlers.get(name) ?? []) handler(payload)
}

const preferences: Preferences = {
  pipeline: {},
  renderer: { font_families: ['CCWildWords'] },
  providers: {},
}

/** The desktop with `open` as its open project, empty. */
function desktop(open: () => string | null) {
  vi.spyOn(commands, 'getOpenProject').mockImplementation(async () => open())
  vi.spyOn(commands, 'getShownPage').mockResolvedValue(null)
  vi.spyOn(commands, 'query').mockImplementation(async (query) =>
    query.query === 'get_project'
      ? { id: 'project', version: '1', pages: [] }
      : { steps: [], current: 0 },
  )
}

beforeEach(() => {
  handlers.clear()
  for (const name of Object.keys(events) as (keyof typeof events)[]) {
    vi.spyOn(events, name).mockImplementation(async (handler: (payload: never) => void) => {
      handlers.set(name, [...(handlers.get(name) ?? []), handler as (payload: unknown) => void])
      return () => undefined
    })
  }
  vi.spyOn(commands, 'listModels').mockResolvedValue([])
  vi.spyOn(commands, 'getPreferences').mockResolvedValue(preferences)
  vi.spyOn(commands, 'getPipelineEvents').mockResolvedValue([])
})

function ProjectProbe() {
  const project = useProject().data
  return createElement(
    'span',
    null,
    project === undefined ? 'Loading' : (project?.name ?? 'Closed'),
  )
}

describe('Tauri runtime', () => {
  it('reads the project that was open before the interface loaded', async () => {
    desktop(() => 'Book')
    const view = render(createElement(Providers, null, createElement(ProjectProbe)))

    expect(await screen.findByText('Book')).toBeInTheDocument()
    view.unmount()
  })

  it('listens once through Strict Mode effect replay', async () => {
    const runs = vi.spyOn(commands, 'getPipelineEvents').mockResolvedValue([])
    const view = render(
      createElement(StrictMode, null, createElement(Providers, null, createElement('div'))),
    )

    await waitFor(() => expect(runs).toHaveBeenCalledTimes(1))
    expect(handlers.get('pipeline')).toHaveLength(1)
    act(() => {
      tell('pipeline', { kind: 'queued', run: 1, steps: 4 })
      tell('pipeline', { kind: 'started', run: 1, page: 'page', stage: 'detect' })
    })

    expect(useKoharuStore.getState().jobs[1]).toMatchObject({
      state: 'running',
      total: 4,
      stage: 'detect',
    })
    view.unmount()
  })

  it('applies the canvas and download events directly to the store', async () => {
    useKoharuStore.setState({ downloads: {} })
    const view = render(createElement(Providers, null, createElement('div')))
    await waitFor(() => expect(commands.getPipelineEvents).toHaveBeenCalledTimes(1))

    act(() => {
      tell('download', { id: 7, state: 'Running', name: 'model.bin', completed: 25, total: 100 })
      tell('camera', { zoom: 0.5, x: 12, y: 34 })
      tell('selection', ['layer'])
    })
    expect(useKoharuStore.getState().downloads[7]).toMatchObject({ completed: 25, total: 100 })
    expect(useKoharuStore.getState().camera).toEqual({ zoom: 0.5, x: 12, y: 34 })
    expect(useKoharuStore.getState().selectedLayers).toEqual(['layer'])
    view.unmount()
  })

  it('shares page data and refreshes only affected pages, keeping event history and navigation', async () => {
    desktop(() => 'Book')
    vi.spyOn(commands, 'getShownPage').mockResolvedValue('a')
    let width = 100
    let releaseSummaries: (() => void) | undefined
    let holdSummaries = false
    const query = vi.spyOn(commands, 'query').mockImplementation(async (request) => {
      const size = { width, height: 100 }
      switch (request.query) {
        case 'get_project':
          if (holdSummaries) await new Promise<void>((resolve) => { releaseSummaries = resolve })
          return { id: 'book', version: '1', pages: ['a', 'b'].map((id) => ({
            id, size, background: null, texts: 0, translated: 0,
          })) }
        case 'get_page':
          return { id: request.page, size, background: null, layers: [] }
        case 'get_story':
          return []
        default:
          return { current: 0, steps: [] }
      }
    })
    function Probe() {
      const project = useProject().data
      const shown = usePage().data
      const a = usePageStory('a').data
      const b = usePageStory('b').data
      return createElement('span', null, [
        shown?.id, a?.page.size.width, b?.page.size.width, project?.history.current,
      ].join(':'))
    }
    const view = render(createElement(Providers, null, createElement(Probe)))
    expect(await screen.findByText('a:100:100:0')).toBeInTheDocument()
    expect(query.mock.calls.filter(([request]) => request.query === 'get_page')).toHaveLength(2)
    query.mockClear()
    vi.mocked(commands.getShownPage).mockClear()
    vi.mocked(commands.getOpenProject).mockClear()

    const change: Change = {
      revision: '2', created: [], layers: ['raster'],
      pages: { added: [], removed: [], changed: ['b'], reordered: false },
      history: { current: 1, steps: ['paint'] },
    }
    width = 120
    holdSummaries = true
    // Two edits arriving together share the same page and summary reads.
    act(() => {
      tell('change', { ...change, revision: '1a' })
      tell('change', change)
    })
    await waitFor(() => expect(releaseSummaries).toBeDefined())
    expect(await screen.findByText('a:100:120:1')).toBeInTheDocument()
    expect(query.mock.calls.map(([request]) => request)).toEqual(expect.arrayContaining([
      { query: 'get_project' }, { query: 'get_page', page: 'b' },
    ]))
    expect(query).toHaveBeenCalledTimes(2)

    // The outstanding summary read must not overwrite a newer page or selection-history event.
    act(() => {
      tell('page', 'b')
      tell('change', {
        ...change, revision: '3', layers: [],
        pages: { added: [], removed: [], changed: [], reordered: false },
        history: { current: 2, steps: ['paint', 'select'] },
      })
      releaseSummaries?.()
    })
    expect(await screen.findByText('b:100:120:2')).toBeInTheDocument()
    await waitFor(() => expect(queryClient.isFetching()).toBe(0))
    expect(screen.getByText('b:100:120:2')).toBeInTheDocument()
    expect(query).toHaveBeenCalledTimes(2)
    expect(commands.getOpenProject).not.toHaveBeenCalled()
    expect(commands.getShownPage).not.toHaveBeenCalled()

    // The style command can finish after the event read. Don't issue a second read.
    await readPage(change, { name: 'Book', page: 'b' })
    expect(query).toHaveBeenCalledTimes(2)

    // Or its response can precede the event. Join that event's scheduled page read.
    holdSummaries = false
    query.mockClear()
    const later = { ...change, revision: '4' }
    const settled = readPage(later, { name: 'Book', page: 'b' })
    act(() => tell('change', later))
    await settled
    await waitFor(() => expect(queryClient.isFetching()).toBe(0))
    expect(query.mock.calls.filter(([request]) => request.query === 'get_page')).toHaveLength(1)
    expect(query.mock.calls.filter(([request]) => request.query === 'get_story')).toHaveLength(0)
    view.unmount()
  })

  it('shares one initial project read with page events and retains newer navigation', async () => {
    let open: string | null = null
    desktop(() => open)
    let releasePage: ((page: string) => void) | undefined
    vi.mocked(commands.getShownPage).mockImplementation(() => new Promise((resolve) => {
      releasePage = resolve
    }))
    const view = render(createElement(Providers, null, createElement(ProjectProbe)))
    expect(await screen.findByText('Closed')).toBeInTheDocument()
    vi.mocked(commands.getOpenProject).mockClear()

    open = 'Book'
    act(() => {
      tell('project', 'Book')
      tell('page', 'a')
    })
    await waitFor(() => expect(releasePage).toBeDefined())
    act(() => {
      tell('page', 'b')
      releasePage?.('a')
    })
    expect(await screen.findByText('Book')).toBeInTheDocument()
    expect(queryClient.getQueryData(projectKey)).toMatchObject({ name: 'Book', page: 'b' })
    expect(commands.getOpenProject).toHaveBeenCalledTimes(1)
    expect(commands.getShownPage).toHaveBeenCalledTimes(1)
    expect(commands.query).toHaveBeenCalledTimes(2)
    view.unmount()
  })

  it('shows a project the desktop tells it opened', async () => {
    let open: string | null = null
    desktop(() => open)
    const view = render(createElement(Providers, null, createElement(ProjectProbe)))
    expect(await screen.findByText('Closed')).toBeInTheDocument()

    open = 'Book'
    act(() => tell('project', 'Book'))

    expect(await screen.findByText('Book')).toBeInTheDocument()
    view.unmount()
  })
})
