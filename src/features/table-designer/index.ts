export { TableDesignerScreen } from './components/designer-screen';
export { useTableDesigner } from './hooks/use-table-designer';
export { useLoadTableDesign, useGenerateTableDdl, useApplyTableDdl, tableDesignerKeys } from './api';
export type { DesignColumn, DesignForeignKey, DesignIndex, TableDesign, DdlStatement, ApplyResult } from './types';
