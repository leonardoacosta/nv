"use client";

import type { ComponentType } from "react";
import type { BriefingBlock } from "@nova/db";

import SectionBlock from "./SectionBlock";
import StatusTable from "./StatusTable";
import MetricCard from "./MetricCard";
import Timeline from "./Timeline";
import ActionGroup from "./ActionGroup";
import KVList from "./KVList";
import AlertBlock from "./AlertBlock";
import SourcePills from "./SourcePills";
import PRList from "./PRList";
import PipelineTable from "./PipelineTable";

// Each renderer receives the typed block directly
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type BlockRenderer = ComponentType<{ block: any; className?: string }>;

function makeSectionRenderer() {
  return function SectionRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "section" }>; className?: string }) {
    return <SectionBlock title={block.title} data={block.data} className={className} />;
  };
}

function makeStatusTableRenderer() {
  return function StatusTableRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "status_table" }>; className?: string }) {
    return <StatusTable title={block.title} data={block.data} className={className} />;
  };
}

function makeMetricCardRenderer() {
  return function MetricCardRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "metric_card" }>; className?: string }) {
    return <MetricCard title={block.title} data={block.data} className={className} />;
  };
}

function makeTimelineRenderer() {
  return function TimelineRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "timeline" }>; className?: string }) {
    return <Timeline title={block.title} data={block.data} className={className} />;
  };
}

function makeActionGroupRenderer() {
  return function ActionGroupRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "action_group" }>; className?: string }) {
    return <ActionGroup title={block.title} data={block.data} className={className} />;
  };
}

function makeKVListRenderer() {
  return function KVListRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "kv_list" }>; className?: string }) {
    return <KVList title={block.title} data={block.data} className={className} />;
  };
}

function makeAlertRenderer() {
  return function AlertRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "alert" }>; className?: string }) {
    return <AlertBlock title={block.title} data={block.data} className={className} />;
  };
}

function makeSourcePillsRenderer() {
  return function SourcePillsRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "source_pills" }>; className?: string }) {
    return <SourcePills title={block.title} data={block.data} className={className} />;
  };
}

function makePRListRenderer() {
  return function PRListRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "pr_list" }>; className?: string }) {
    return <PRList title={block.title} data={block.data} className={className} />;
  };
}

function makePipelineTableRenderer() {
  return function PipelineTableRenderer({ block, className }: { block: Extract<BriefingBlock, { type: "pipeline_table" }>; className?: string }) {
    return <PipelineTable title={block.title} data={block.data} className={className} />;
  };
}

export const BlockRegistry: Record<string, BlockRenderer> = {
  section: makeSectionRenderer(),
  status_table: makeStatusTableRenderer(),
  metric_card: makeMetricCardRenderer(),
  timeline: makeTimelineRenderer(),
  action_group: makeActionGroupRenderer(),
  kv_list: makeKVListRenderer(),
  alert: makeAlertRenderer(),
  source_pills: makeSourcePillsRenderer(),
  pr_list: makePRListRenderer(),
  pipeline_table: makePipelineTableRenderer(),
};

interface BriefingRendererProps {
  blocks: BriefingBlock[];
  className?: string;
}

export function BriefingRenderer({ blocks, className }: BriefingRendererProps) {
  return (
    <div className={`space-y-4 ${className ?? ""}`}>
      {blocks.map((block, i) => {
        const Renderer = BlockRegistry[block.type];
        if (!Renderer) return null;
        return <Renderer key={i} block={block} />;
      })}
    </div>
  );
}
