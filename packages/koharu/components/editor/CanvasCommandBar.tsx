'use client'

import { Eye, EyeOff } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { InferenceControl } from '@/components/editor/InferenceControl'
import { send, usePage } from '@/lib/queries'
import { pipelineStages, useKoharuStore, type PipelineScope } from '@/lib/store'
import { useTypeBarWorkspace } from '@/lib/typeBarWorkspace'
import { commands, type Stage, type Target } from '@koharu/ipc/protocol'

export function CanvasCommandBar() {
  const { t } = useTranslation()
  const hidden = useTypeBarWorkspace((state) => state.hidden)
  const setHidden = useTypeBarWorkspace((state) => state.setHidden)
  const page = usePage().data
  const selectedPages = useKoharuStore((state) => state.selectedPages)
  const jobs = useKoharuStore((state) => state.jobs)
  const running = Object.values(jobs).find((job) => job.state === 'running')

  const run = (selection: PipelineScope, stages: Stage[]) => {
    const target: Target = selection === 'project' ? 'project' : { pages: selectedPages }
    // A run takes its stages in the pipeline's order, whichever order they were picked in.
    const ordered = pipelineStages.filter((stage) => stages.includes(stage))
    send(commands.runPipeline, target, ordered)
  }

  return (
    <header className='flex h-10 shrink-0 items-center gap-2 border-b border-border/80 bg-[var(--surface-toolbar)] px-2.5'>
      <div className='min-w-0 flex-1' />
      <button
        type='button'
        className='grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-foreground/[0.08] hover:text-foreground disabled:opacity-50'
        disabled={!page}
        title={t(hidden ? 'typeBarWorkspace.show' : 'typeBarWorkspace.hide')}
        aria-label={t(hidden ? 'typeBarWorkspace.show' : 'typeBarWorkspace.hide')}
        onClick={() => setHidden(!hidden)}
      >
        {hidden ? <EyeOff className='size-4' /> : <Eye className='size-4' />}
      </button>
      <InferenceControl disabled={!page || Boolean(running)} onRun={run} />
    </header>
  )
}
