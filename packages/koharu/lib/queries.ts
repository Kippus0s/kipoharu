'use client'

import {
  MutationCache,
  MutationObserver,
  QueryCache,
  QueryClient,
  type QueryKey,
  queryOptions,
  useIsMutating,
  useMutation,
  useQuery,
} from '@tanstack/react-query'
import type { ParseKeys } from 'i18next'
import { useDebouncedCallback } from 'use-debounce'

import {
  commands,
  type Change,
  type FontFamily,
  type History,
  type Page,
  type PageId,
  type PageSummary,
  type Preferences,
  type ProjectAnswer,
} from '@koharu/ipc/protocol'
import { toast } from '@koharu/ui/components/toast'

import { storyOrder } from './document'
import i18n from './i18n'

export const projectKey = ['project'] as const
export const pageKey = ['page'] as const
/** Page data is shared by the canvas inspector and Story, keyed by project and page. */
export const pageDataKey = (name: string | null, page: PageId | null) =>
  [...pageKey, name, page] as const
export const projectsKey = ['projects'] as const
export const fontsKey = ['fonts', 'installed'] as const
export const onlineFontsKey = ['fonts', 'online'] as const
export const preferencesKey = ['preferences'] as const
export const modelsKey = ['models'] as const

/** The open project as the editor lists it: its name, the page the canvas shows, its pages, and
 * its history, read through the scene's own queries. */
export interface ProjectView {
  name: string
  page: PageId | null
  pages: PageSummary[]
  history: History
  /** Each affected page's last revision, which its thumbnail follows. */
  revisions: Record<PageId, string>
}

declare module '@tanstack/react-query' {
  interface Register {
    /** `activity` is the activity center's row while the mutation runs. */
    mutationMeta: { activity?: ParseKeys }
  }
}

/** A failed command's toast: the desktop rejects with its error's message. */
function report(error: unknown): void {
  const description =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : i18n.t('errors.unknown')
  toast.add({ type: 'error', title: i18n.t('errors.actionFailed'), description })
}

/** Every command runs as a query or a mutation, whose failure shows here. Commands go to the
 * desktop, not the network, so they run whether or not the browser reports being online. */
export const queryClient = new QueryClient({
  queryCache: new QueryCache({ onError: report }),
  mutationCache: new MutationCache({ onError: report }),
  defaultOptions: {
    queries: {
      networkMode: 'always',
      staleTime: Number.POSITIVE_INFINITY,
      retry: false,
      refetchOnReconnect: false,
      refetchOnWindowFocus: false,
    },
    mutations: { networkMode: 'always' },
  },
})

/** Runs a command whose answer nothing waits on, as a mutation, so a failure shows as any
 * mutation's does. */
export function send<Args extends unknown[]>(
  command: (...args: Args) => Promise<unknown>,
  ...args: Args
): void {
  // TanStack Query never drops a mutation while an observer holds it, and nothing reads a
  // settled command, so let go of it once it settles and let the cache drop it right away.
  const observer = new MutationObserver(queryClient, {
    mutationFn: () => command(...args),
    gcTime: 0,
  })
  void observer
    .mutate()
    .catch(() => undefined)
    .finally(() => observer.reset())
}

const projectQuery = queryOptions({
  queryKey: projectKey,
  queryFn: async (): Promise<ProjectView | null> => {
    // Changes and page events carry the history and shown page. Only the summaries
    // need to be fetched again once the project is known. Merge into the latest cache
    // value, since a selection or page event may arrive while the summaries load.
    const cached = queryClient.getQueryData<ProjectView | null>(projectKey)
    if (cached) {
      const project = (await commands.query({ query: 'get_project' })) as ProjectAnswer
      const latest = queryClient.getQueryData<ProjectView | null>(projectKey)
      return latest ? { ...latest, pages: project.pages } : null
    }
    const name = await commands.getOpenProject()
    if (name === null) return null
    const [page, project, history] = await Promise.all([
      commands.getShownPage(),
      commands.query({ query: 'get_project' }),
      commands.query({ query: 'get_history' }),
    ])
    const known = queryClient.getQueryData<ProjectView | null>(projectKey)
    return {
      name,
      page,
      pages: (project as ProjectAnswer).pages,
      history: history as History,
      revisions: known?.name === name ? known.revisions : {},
    }
  },
})

const pageQuery = (name: string | null, page: PageId | null) =>
  queryOptions({
    queryKey: pageDataKey(name, page),
    queryFn: async (): Promise<Page | null> => {
      if (name === null || page === null) return null
      return (await commands.query({ query: 'get_page', page })) as Page
    },
  })

const fontsQuery = queryOptions({
  queryKey: fontsKey,
  queryFn: () => commands.listInstalledFonts(),
})

const onlineFontsQuery = queryOptions({
  queryKey: onlineFontsKey,
  queryFn: () => commands.listOnlineFonts(),
})

/** The open project; changes supply history and page events supply the shown page. */
export function useProject(enabled = true) {
  return useQuery({ ...projectQuery, enabled })
}

/** The open project's pages, in order. */
export function usePages(enabled = true) {
  return { data: useProject(enabled).data?.pages }
}

/** The page the editor shows, with its layers. */
export function usePage(enabled = true) {
  const { data: shown } = useQuery({
    ...projectQuery,
    enabled,
    select: (project) => [project?.name ?? null, project?.page ?? null] as const,
  })
  const [name, page] = shown ?? [null, null]
  return useQuery({ ...pageQuery(name, page), enabled: enabled && page !== null })
}

/** Story's order is a derived view of the already shared page. React Query memoizes
 * the stable selector and structurally shares unchanged results, with no second IPC read. */
const selectStory = (page: Page | null) =>
  page ? { page, story: storyOrder(page.layers) } : null

export function usePageStory(page: PageId, enabled = true) {
  const { data: name = null } = useQuery({
    ...projectQuery,
    enabled,
    select: (project) => project?.name ?? null,
  })
  return useQuery({ ...pageQuery(name, page), enabled: enabled && name !== null, select: selectStory })
}

/** Installed fonts only; looking at this list never downloads the online catalog. */
export function useFonts(enabled = true) {
  return useQuery({ ...fontsQuery, enabled })
}

/** The online catalog is fetched only if a picker opens its Online Fonts tab. */
export function useOnlineFonts(enabled = false) {
  return useQuery({ ...onlineFontsQuery, enabled })
}

export function usePreferences() {
  return useQuery({ queryKey: preferencesKey, queryFn: () => commands.getPreferences() })
}

/** Saves the preferences one save after another. Each save shows at once; what the file holds
 * replaces it only while the cache still holds that save, so an answer never undoes a newer
 * edit, saved or still waiting to be. */
export function useSavePreferences() {
  return useMutation({
    mutationKey: preferencesKey,
    scope: { id: 'preferences' },
    mutationFn: (preferences: Preferences) => commands.savePreferences(preferences),
    onMutate: async (preferences) => {
      await queryClient.cancelQueries({ queryKey: preferencesKey })
      queryClient.setQueryData(preferencesKey, preferences)
    },
    onSettled: (saved, _, preferences) => {
      if (queryClient.getQueryData(preferencesKey) !== preferences) return
      if (saved) queryClient.setQueryData(preferencesKey, saved)
      else void queryClient.invalidateQueries({ queryKey: preferencesKey })
    },
  })
}

/** Edits the preferences as a settings page does: each edit shows at once, and saves once the
 * edits pause; `flush` saves one still waiting. */
export function useEditPreferences() {
  const { mutate } = useSavePreferences()
  const save = useDebouncedCallback(mutate, 260, { flushOnExit: true })
  const edit = (preferences: Preferences) => {
    void queryClient.cancelQueries({ queryKey: preferencesKey })
    queryClient.setQueryData(preferencesKey, preferences)
    save(preferences)
  }
  return { edit, flush: save.flush }
}

/** The models the translator can use. */
export function useModels() {
  return useQuery({ queryKey: modelsKey, queryFn: () => commands.listModels() })
}

/** The projects the start view lists, read again each time it shows. */
export function useProjects() {
  return useQuery({
    queryKey: projectsKey,
    queryFn: () => commands.listProjects(),
    refetchOnMount: 'always',
  })
}

export function useFontPreview(font: FontFamily | undefined, enabled = true) {
  return useQuery({
    queryKey: ['font-preview', font?.name],
    queryFn: async () => {
      if (!font) return null
      try {
        return new Uint8Array(await commands.getFontPreview(font.name))
      } catch {
        return null
      }
    },
    enabled: enabled && font !== undefined,
    gcTime: 5 * 60 * 1000,
  })
}

/** A command the activity center lists by `label` while it runs, busy wherever it's used. */
export function useCommand<Args extends unknown[], Result>(
  key: QueryKey,
  command: (...args: Args) => Promise<Result>,
  label: ParseKeys,
) {
  const busy = useIsMutating({ mutationKey: key }) > 0
  const mutation = useMutation({
    mutationKey: key,
    mutationFn: (args: Args) => command(...args),
    meta: { activity: label },
  })
  return { run: (...args: Args) => mutation.mutate(args), busy }
}

/** Imports pages; the project's change arrives as an event. */
export function useImportPages() {
  const { run, busy } = useCommand(['import-pages'], commands.import, 'navigator.importing')
  return { importPages: run, importing: busy }
}

/** Invalidate immediately, but share one active read per key for a short burst of events.
 * Never let a read started before a change repopulate the cache. Inactive pages stay stale
 * until they are next viewed. Separate document edits and undo steps are not merged. */
const queuedRefreshes = new WeakMap<object, Promise<void>>()

function pendingRefresh(queryKey: QueryKey): Promise<void> | undefined {
  const query = queryClient.getQueryCache().find({ queryKey, exact: true })
  return query && queuedRefreshes.get(query)
}

export function queueRefresh(queryKey: QueryKey): Promise<void> {
  const filters = { queryKey, exact: true }
  const query = queryClient.getQueryCache().find(filters)
  if (!query) return Promise.resolve()
  void queryClient.cancelQueries(filters)
  void queryClient.invalidateQueries({ ...filters, refetchType: 'none' })
  const queued = queuedRefreshes.get(query)
  if (queued) return queued
  const promise = new Promise<void>((resolve) => {
    setTimeout(() => {
      queuedRefreshes.delete(query)
      // A project switch or page removal may have discarded this cache entry.
      if (queryClient.getQueryCache().find(filters) !== query) return resolve()
      void queryClient.refetchQueries({ ...filters, type: 'active' }, { cancelRefetch: false })
        .then(resolve, resolve)
    }, 16)
  })
  queuedRefreshes.set(query, promise)
  return promise
}

/** Keep the TypeBar's draft until its edit's event-driven page read finishes. If the event
 * arrived before the command answered, do not read again. If it arrives later, join its read.
 * A bounded fallback covers delayed/missed events without leaving the control pending forever. */
export async function readPage(
  change: Change,
  origin: Pick<ProjectView, 'name' | 'page'>,
): Promise<void> {
  const { name, page } = origin
  if (!page || !change.pages.changed.includes(page)) return
  const queryKey = pageDataKey(name, page)
  const filters = { queryKey, exact: true }
  const project = () => queryClient.getQueryData<ProjectView | null>(projectKey)
  const observed = () => project()?.name !== name || project()?.revisions[page] === change.revision
  if (!observed()) {
    await new Promise<void>((resolve) => {
      const finish = () => {
        clearTimeout(timer)
        unsubscribe()
        resolve()
      }
      const unsubscribe = queryClient.getQueryCache().subscribe(() => {
        if (observed()) finish()
      })
      const timer = setTimeout(finish, 500)
    })
  }
  if (project()?.name !== name || project()?.page !== page) return
  const queued = pendingRefresh(queryKey)
  if (queued) await queued
  else if (!observed()) await queryClient.refetchQueries(filters, { cancelRefetch: false })
  while (project()?.name === name && project()?.page === page) {
    const next = pendingRefresh(queryKey)
    if (!next && queryClient.isFetching(filters) === 0) break
    if (next) await next
    else await queryClient.refetchQueries(filters, { cancelRefetch: false })
  }
}

export async function refresh(...keys: QueryKey[]): Promise<void> {
  await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })))
}
