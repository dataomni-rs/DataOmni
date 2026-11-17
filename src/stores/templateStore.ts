import { create } from 'zustand';
import {
  loadQueryTemplates,
  removeQueryTemplate,
  saveQueryTemplates,
  upsertQueryTemplate,
  type QueryTemplate,
  type QueryTemplateDraft,
} from '../utils/queryTemplates';

interface TemplateState {
  templates: QueryTemplate[];
  saveTemplate: (draft: QueryTemplateDraft) => void;
  removeTemplate: (id: string) => void;
}

export const useTemplateStore = create<TemplateState>((set) => ({
  templates: loadQueryTemplates(),

  saveTemplate: (draft) => {
    set((state) => {
      const templates = upsertQueryTemplate(state.templates, draft);
      saveQueryTemplates(templates);
      return { templates };
    });
  },

  removeTemplate: (id) => {
    set((state) => {
      const templates = removeQueryTemplate(state.templates, id);
      saveQueryTemplates(templates);
      return { templates };
    });
  },
}));
