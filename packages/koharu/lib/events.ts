'use client'

import { commands, events, type Change, type PageId } from '@koharu/ipc/protocol'
import { toast } from '@koharu/ui/components/toast'

import { toHex } from './color'
import i18n from './i18n'
import {
  pageDataKey,
  pageKey,
  projectKey,
  queryClient,
  refresh,
  storyKey,
  storyPageKey,
  type ProjectView,
} from './queries'
import {
  receiveCanvasSelection,
  receiveDownload,
  receivePickedColor,
  receivePipeline,
  receiveRuns,
  useKoharuStore,
} from './store'

/** Hears what the desktop tells for the interface's life, while `active` holds, which it doesn't
 * while React replays an effect: listening first, then reading the runs so far, so nothing told
 * while they load is missed. */
export function hear(active: () => boolean): Promise<void> {
  const listen = <T>(
    event: (handler: (payload: T) => void) => Promise<unknown>,
    receive: (payload: T) => void,
  ) =>
    event((payload) => {
      if (active()) receive(payload)
    })

  return Promise.all([
    listen(events.change, receiveChange),
    listen(events.project, receiveProject),
    listen(events.page, receivePage),
    listen(events.selection, receiveCanvasSelection),
    listen(events.camera, (camera) => useKoharuStore.setState({ camera })),
    listen(events.selectionBounds, (selectionBounds) =>
      useKoharuStore.setState({ selectionBounds }),
    ),
    listen(events.selectionScales, (selectionScales) => useKoharuStore.setState({ selectionScales })),
    listen(events.area, (area) => useKoharuStore.setState({ area })),
    listen(events.typing, (typing) => useKoharuStore.setState({ typing })),
    listen(events.color, (color) => receivePickedColor(toHex(color))),
    listen(events.brushDiameter, (diameter) => {
      const store = useKoharuStore.getState()
      store.setBrush({ ...store.brush, diameter })
    }),
    listen(events.notice, (notice) =>
      toast.add({ type: 'info', title: i18n.t(`canvas.notice.${notice}`) }),
    ),
    listen(events.pipeline, receivePipeline),
    listen(events.download, receiveDownload),
  ])
    .then(() => commands.getPipelineEvents())
    .then((runs) => {
      if (active()) receiveRuns(runs)
    })
}

/** Cancel even an initial read, whose result might predate the event. */
function rereadProject(): void {
  void queryClient.cancelQueries({ queryKey: projectKey, exact: true }, { revert: false })
  void refresh(projectKey).catch(() => undefined)
}

/** Another project opened or closed, or a notification was missed. In all cases the
 * old page data and reading order must not survive a full resynchronisation. */
function receiveProject(name: string | null): void {
  const previous = queryClient.getQueryData<ProjectView | null>(projectKey)
  if (previous?.name !== name) {
    const store = useKoharuStore.getState()
    store.selectPages([])
    store.selectLayers([])
  }
  void queryClient.cancelQueries({ queryKey: projectKey, exact: true }, { revert: false })
  for (const queryKey of [pageKey, storyKey, ['thumbnail']]) {
    queryClient.removeQueries({ queryKey })
  }
  if (name === null) {
    queryClient.setQueryData(projectKey, null)
  } else {
    // Clear the previous view so the next query reads name, shown page and history again.
    void queryClient.resetQueries({ queryKey: projectKey, exact: true }).catch(() => undefined)
  }
}

/** The page event is authoritative; changing page switches the shared page query's key. */
function receivePage(page: PageId | null): void {
  const project = queryClient.getQueryData<ProjectView | null>(projectKey)
  if (project) {
    queryClient.setQueryData<ProjectView>(projectKey, { ...project, page })
  } else if (page !== null) {
    rereadProject()
  }
  if (page !== null && useKoharuStore.getState().selectedPages.length === 0) {
    useKoharuStore.getState().selectPages([page])
  }
}

/** Edits name the affected pages, even when they aren't shown. History comes with the
 * event; page and Story data for unaffected pages remain valid. */
function receiveChange(change: Change): void {
  const project = queryClient.getQueryData<ProjectView | null>(projectKey)
  if (!project) {
    rereadProject()
    return
  }
  const affected = [...change.pages.added, ...change.pages.changed]
  const revisions = { ...project.revisions }
  for (const page of affected) revisions[page] = change.revision
  for (const page of change.pages.removed) delete revisions[page]
  queryClient.setQueryData<ProjectView>(projectKey, {
    ...project,
    history: change.history,
    revisions,
  })

  for (const page of change.pages.removed) {
    for (const queryKey of [pageDataKey(project.name, page), storyPageKey(project.name, page)]) {
      queryClient.removeQueries({ queryKey, exact: true })
    }
  }
  for (const page of affected) {
    for (const queryKey of [pageDataKey(project.name, page), storyPageKey(project.name, page)]) {
      // Also cancel inactive or first-time reads; their old answers must not repopulate the cache.
      void queryClient.cancelQueries({ queryKey, exact: true })
      void queryClient.invalidateQueries({ queryKey, exact: true }).catch(() => undefined)
    }
  }
  if (affected.length || change.pages.removed.length || change.pages.reordered) {
    void refresh(projectKey).catch(() => undefined)
  }
}
