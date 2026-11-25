'use client'

import { openUrl } from '@tauri-apps/plugin-opener'
import { FilePlus2, FolderOpen, LoaderCircle } from 'lucide-react'
import Image from 'next/image'
import { Fragment, useState, type ComponentProps } from 'react'
import { useTranslation } from 'react-i18next'

import { AboutDialog } from '@/components/app/AboutDialog'
import { useMacOS, WindowControls } from '@/components/app/WindowChrome'
import { send, useCommand, useImportPages, usePage, useProject } from '@/lib/queries'
import { deleteSelection } from '@/lib/scene'
import { shortcutLabel, shortcuts } from '@/lib/shortcuts'
import { useKoharuStore } from '@/lib/store'
import { commands, type ExportFormat, type PageId } from '@koharu/ipc/protocol'
import { Button } from '@koharu/ui/components/button'
import { Checkbox } from '@koharu/ui/components/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@koharu/ui/components/dialog'
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarShortcut,
  MenubarSub,
  MenubarSubContent,
  MenubarSubTrigger,
  MenubarTrigger as UiMenubarTrigger,
} from '@koharu/ui/components/menubar'
import { cn } from '@koharu/ui/lib/utils'

const exportFormats = ['png', 'psd', 'cbz'] as const

export function TitleBar() {
  const { t } = useTranslation()
  const [aboutOpen, setAboutOpen] = useState(false)
  const [batchOpen, setBatchOpen] = useState(false)
  const [batchPages, setBatchPages] = useState<PageId[]>([])
  const [batchFormats, setBatchFormats] = useState<ExportFormat[]>(['png'])
  const macOS = useMacOS()
  const project = useProject().data
  const pageQuery = usePage(Boolean(project))
  const pages = project?.pages ?? []
  const page = project ? (pageQuery.data ?? null) : null
  const pageNumber = page ? pages.findIndex((candidate) => candidate.id === page.id) + 1 : 0
  const activePage = project?.page ?? null
  const selectedPages = useKoharuStore((state) => state.selectedPages)
  const validBatchPages = pages.filter((candidate) => batchPages.includes(candidate.id))
  const history = project?.history
  const historyOpen = useKoharuStore((state) => state.historyOpen)
  const setHistoryOpen = useKoharuStore((state) => state.setHistoryOpen)
  const selectedLayers = useKoharuStore((state) => state.selectedLayers)
  const selectLayers = useKoharuStore((state) => state.selectLayers)
  const setSettingsOpen = useKoharuStore((state) => state.setSettingsOpen)
  const { importPages, importing } = useImportPages()
  const { run: exportProject, busy: exporting } = useCommand(
    ['export-project'],
    commands.export,
    'menu.export',
  )

  const psdLabels = {
    background: t('psd.background'),
    cleanup: t('psd.cleanup'),
    outline: t('psd.outline'),
  }

  const openBatch = () => {
    const initial = pages
      .filter((candidate) => selectedPages.includes(candidate.id))
      .map((candidate) => candidate.id)
    setBatchPages(initial.length ? initial : activePage ? [activePage] : [])
    setBatchFormats(['png'])
    setBatchOpen(true)
  }

  return (
    <>
      <header
        data-tauri-drag-region='deep'
        className='relative flex h-10 shrink-0 items-center bg-[var(--surface-titlebar)] text-[12px]'
      >
        {macOS ? (
          <div className='w-[84px] shrink-0' />
        ) : (
          <div className='relative z-10 flex h-full w-10 shrink-0 items-center justify-center rounded-br-lg'>
            {/* Its weight is in its top, its feet pale, so centred it reads 2 px above the
                menus' words; lowered by that to sit on their line. */}
            <Image
              className='pointer-events-none translate-y-0.5'
              src='/icon.png'
              alt='Koharu'
              width={17}
              height={17}
              draggable={false}
              priority
            />
          </div>
        )}
        <Menubar className='relative z-10 h-full shrink-0 gap-0 border-0 bg-transparent p-0 shadow-none'>
          <MenubarMenu>
            <MenubarTrigger>{t('menu.file')}</MenubarTrigger>
            <MenubarContent>
              <MenubarSub>
                <MenubarSubTrigger disabled={!project || importing} aria-busy={importing}>
                  {importing && <LoaderCircle className='animate-spin' aria-hidden='true' />}
                  {importing ? t('navigator.importing') : t('menu.import')}
                </MenubarSubTrigger>
                <MenubarSubContent>
                  <MenubarItem disabled={importing} onClick={() => importPages('files')}>
                    <FilePlus2 />
                    {t('navigator.importFiles')}
                  </MenubarItem>
                  <MenubarItem disabled={importing} onClick={() => importPages('folder')}>
                    <FolderOpen />
                    {t('navigator.importFolder')}
                  </MenubarItem>
                </MenubarSubContent>
              </MenubarSub>
              <MenubarSub>
                <MenubarSubTrigger
                  disabled={!project || pages.length === 0 || exporting}
                  aria-busy={exporting}
                >
                  {exporting && <LoaderCircle className='animate-spin' aria-hidden='true' />}
                  {t('menu.export')}
                </MenubarSubTrigger>
                <MenubarSubContent>
                  {exportFormats.map((format) => (
                    <Fragment key={format}>
                      <MenubarItem
                        disabled={exporting}
                        onClick={() => exportProject([format], psdLabels, null)}
                      >
                        {format.toUpperCase()}…
                      </MenubarItem>
                      <MenubarItem
                        disabled={exporting || !activePage}
                        onClick={() => {
                          if (activePage) exportProject([format], psdLabels, [activePage])
                        }}
                      >
                        {t('menu.exportSelectedPage', { format: format.toUpperCase() })}
                      </MenubarItem>
                    </Fragment>
                  ))}
                  <MenubarSeparator />
                  <MenubarItem disabled={exporting} onClick={openBatch}>
                    {t('menu.exportBatch')}
                  </MenubarItem>
                </MenubarSubContent>
              </MenubarSub>
              <MenubarSeparator />
              <MenubarItem disabled={!project} onClick={() => send(commands.close)}>
                {t('menu.closeProject')}
              </MenubarItem>
              <MenubarSeparator />
              <MenubarItem onClick={() => setSettingsOpen(true)}>{t('menu.settings')}</MenubarItem>
            </MenubarContent>
          </MenubarMenu>

          <MenubarMenu>
            <MenubarTrigger>{t('menu.edit')}</MenubarTrigger>
            <MenubarContent>
              <MenubarItem
                disabled={!history || history.current === 0}
                onClick={() => send(commands.undo)}
              >
                {t('menu.undo')}
                <MenubarShortcut>{shortcutLabel(shortcuts.undo)}</MenubarShortcut>
              </MenubarItem>
              <MenubarItem
                disabled={!history || history.current >= history.steps.length}
                onClick={() => send(commands.redo)}
              >
                {t('menu.redo')}
                <MenubarShortcut>{shortcutLabel(shortcuts.redo)}</MenubarShortcut>
              </MenubarItem>
              <MenubarSeparator />
              <MenubarItem
                disabled={!page}
                onClick={() => selectLayers(page?.layers.map((layer) => layer.id) ?? [])}
              >
                {t('menu.selectAllLayers')}
                <MenubarShortcut>{shortcutLabel(shortcuts.selectAll)}</MenubarShortcut>
              </MenubarItem>
              <MenubarItem
                disabled={selectedLayers.length === 0}
                variant='destructive'
                onClick={() => send(deleteSelection, selectedLayers, page?.layers ?? [])}
              >
                {t('menu.delete')}
                <MenubarShortcut>{shortcutLabel(shortcuts.delete)}</MenubarShortcut>
              </MenubarItem>
              <MenubarSeparator />
              {/* Frees the GPU, as Photoshop's Edit → Purge frees memory. */}
              <MenubarItem onClick={() => send(commands.unload)}>
                {t('menu.unloadModels')}
              </MenubarItem>
            </MenubarContent>
          </MenubarMenu>

          <MenubarMenu>
            <MenubarTrigger>{t('menu.view')}</MenubarTrigger>
            <MenubarContent>
              <MenubarItem disabled={!page} onClick={() => send(commands.fit)}>
                {t('menu.fit')}
                <MenubarShortcut>{shortcutLabel(shortcuts.fit)}</MenubarShortcut>
              </MenubarItem>
              <MenubarCheckboxItem
                disabled={!page}
                checked={historyOpen}
                onCheckedChange={setHistoryOpen}
                closeOnClick
              >
                {t('menu.history')}
              </MenubarCheckboxItem>
            </MenubarContent>
          </MenubarMenu>

          <MenubarMenu>
            <MenubarTrigger>{t('menu.help')}</MenubarTrigger>
            <MenubarContent>
              <MenubarItem
                onClick={() =>
                  void openUrl('https://github.com/koharu-rs/koharu').catch(() => undefined)
                }
              >
                {t('menu.github')}
              </MenubarItem>
              <MenubarItem
                onClick={() => void openUrl('https://discord.gg/mHvHkxGnUY').catch(() => undefined)}
              >
                {t('menu.discord')}
              </MenubarItem>
              <MenubarSeparator />
              <MenubarItem
                onClick={() =>
                  void openUrl('https://github.com/sponsors/mayocream').catch(() => undefined)
                }
              >
                {t('menu.donate')}
              </MenubarItem>
              <MenubarSeparator />
              <MenubarItem onClick={() => setAboutOpen(true)}>{t('menu.about')}</MenubarItem>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>

        <div className='min-w-0 flex-1' />
        <div className='pointer-events-none absolute inset-y-0 left-1/2 flex max-w-[40vw] min-w-16 -translate-x-1/2 items-center justify-center px-3 text-[11px] text-muted-foreground select-none'>
          {project ? (
            <span className='truncate'>
              <span className='font-medium text-foreground'>{project.name}</span>
              {page && (
                <>
                  <span className='mx-2'>/</span>
                  <span>{t('navigator.pageNumber', { number: pageNumber })}</span>
                </>
              )}
            </span>
          ) : (
            <span>Koharu</span>
          )}
        </div>

        {!macOS && <WindowControls />}
      </header>
      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />
      <Dialog open={batchOpen && Boolean(project)} onOpenChange={setBatchOpen}>
        <DialogContent className='max-h-[90vh] min-h-0 grid-rows-[auto_auto_auto_minmax(0,1fr)_auto] sm:max-w-md'>
          <DialogHeader>
            <DialogTitle>{t('exportBatch.title')}</DialogTitle>
            <DialogDescription>{t('exportBatch.description')}</DialogDescription>
          </DialogHeader>
          <fieldset className='flex flex-wrap gap-5 rounded-lg border p-3'>
            <legend className='px-1 text-xs font-medium'>{t('exportBatch.formats')}</legend>
            {exportFormats.map((format) => (
              <label key={format} className='flex cursor-pointer items-center gap-2 text-sm'>
                <Checkbox
                  checked={batchFormats.includes(format)}
                  onCheckedChange={(checked) =>
                    setBatchFormats((current) =>
                      checked ? [...current, format] : current.filter((item) => item !== format),
                    )
                  }
                  aria-label={format.toUpperCase()}
                />
                {format.toUpperCase()}
              </label>
            ))}
          </fieldset>
          <div className='flex flex-wrap items-center gap-1'>
            <span className='mr-auto text-xs font-medium'>{t('navigator.pages')}</span>
            <Button
              size='xs'
              variant='outline'
              onClick={() => setBatchPages(pages.map((item) => item.id))}
            >
              {t('exportBatch.selectAll')}
            </Button>
            <Button size='xs' variant='outline' onClick={() => setBatchPages([])}>
              {t('exportBatch.deselectAll')}
            </Button>
            <Button
              size='xs'
              variant='outline'
              onClick={() =>
                setBatchPages((current) =>
                  pages.filter((item) => !current.includes(item.id)).map((item) => item.id),
                )
              }
            >
              {t('exportBatch.invert')}
            </Button>
          </div>
          <div className='min-h-0 max-h-[40vh] overflow-y-auto rounded-lg border p-1'>
            {pages.map((candidate, index) => (
              <label
                key={candidate.id}
                className='flex cursor-pointer items-center gap-3 rounded-md px-3 py-2 hover:bg-muted'
              >
                <Checkbox
                  checked={batchPages.includes(candidate.id)}
                  onCheckedChange={(checked) =>
                    setBatchPages((current) =>
                      checked
                        ? [...current, candidate.id]
                        : current.filter((id) => id !== candidate.id),
                    )
                  }
                  aria-label={t('navigator.pageNumber', { number: index + 1 })}
                />
                {t('navigator.pageNumber', { number: index + 1 })}
              </label>
            ))}
          </div>
          <DialogFooter className='items-center'>
            <span className='mr-auto text-xs text-muted-foreground tabular-nums' role='status'>
              {t('exportBatch.selected', { count: validBatchPages.length, total: pages.length })}
            </span>
            <Button variant='outline' onClick={() => setBatchOpen(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              disabled={exporting || validBatchPages.length === 0 || batchFormats.length === 0}
              onClick={() => {
                setBatchOpen(false)
                exportProject(
                  batchFormats,
                  psdLabels,
                  validBatchPages.map((candidate) => candidate.id),
                )
              }}
            >
              {t('exportBatch.export')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function MenubarTrigger({ className, ...props }: ComponentProps<typeof UiMenubarTrigger>) {
  return (
    <UiMenubarTrigger
      className={cn(
        'h-6 px-1.5 text-[11px] text-muted-foreground transition-colors hover:bg-primary/10 hover:text-muted-foreground aria-expanded:bg-primary/10 aria-expanded:text-muted-foreground',
        className,
      )}
      {...props}
    />
  )
}
