/// <reference types="D:/kudi-project/templates/ai-pm/ai-frontend/node_modules/.pnpm/@vue+language-core@3.3.13/node_modules/@vue/language-core/types/template-helpers.d.ts" />
/// <reference types="D:/kudi-project/templates/ai-pm/ai-frontend/node_modules/.pnpm/@vue+language-core@3.3.13/node_modules/@vue/language-core/types/props-fallback.d.ts" />
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import PageFrame from "@/components/shell/PageFrame.vue";
import { confirmKbRevision, getKbDocument, listKbDocuments, listPcObjects } from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
/**
 * 知识库（KB 文档 + PC 认知对象）——/project/knowledge 落地页。
 *
 * - KB 页签：文档列表（最新修订投影）+ 详情（正文 / 修订历史 / 草稿确认发布）。
 *   确认门在后端（真人 interactive 会话；AI/API Key 403）——按钮仅是入口。
 * - PC 页签：认知对象列表（生命周期徽章 + 最新修订摘要）。
 */
const { t } = useI18n();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);
const activeTab = ref("kb");
const loading = ref(false);
const errorText = ref("");
const kbRows = ref([]);
const pcRows = ref([]);
/** 展开的文档详情（按 document_id 缓存，避免来回切换重复拉取）。 */
const detail = ref(null);
const detailLoading = ref(false);
const confirming = ref(false);
const confirmedNotice = ref("");
const canLoad = computed(() => Boolean(currentProjectId.value));
async function loadLists() {
    if (!canLoad.value)
        return;
    loading.value = true;
    errorText.value = "";
    try {
        const projectId = currentProjectId.value;
        const [kb, pc] = await Promise.all([
            listKbDocuments(projectId),
            listPcObjects(projectId)
        ]);
        kbRows.value = kb.items;
        pcRows.value = pc.items;
    }
    catch {
        errorText.value = t("s01.knowledge.error.load");
    }
    finally {
        loading.value = false;
    }
}
async function openDetail(row) {
    if (!canLoad.value)
        return;
    detailLoading.value = true;
    confirmedNotice.value = "";
    try {
        detail.value = await getKbDocument(row.document_id, currentProjectId.value);
    }
    catch {
        errorText.value = t("s01.knowledge.error.load");
    }
    finally {
        detailLoading.value = false;
    }
}
async function confirmPublish(row) {
    if (!canLoad.value || confirming.value)
        return;
    confirming.value = true;
    confirmedNotice.value = "";
    try {
        // 确认对象 = 最新**草稿**修订（不是 revisions[0]——那可能是已发布修订）。
        const draft = detail.value?.revisions.find(r => r.status === "draft");
        if (!draft)
            return;
        await confirmKbRevision(row.document_id, draft.revision_id, currentProjectId.value);
        confirmedNotice.value = t("s01.knowledge.notice.confirmed");
        // 确认后刷新列表与详情（状态徽章 / 确认人即时可见）。
        await loadLists();
        if (detail.value) {
            detail.value = await getKbDocument(row.document_id, currentProjectId.value);
        }
    }
    catch {
        confirmedNotice.value = t("s01.knowledge.error.confirm");
    }
    finally {
        confirming.value = false;
    }
}
function closeDetail() {
    detail.value = null;
    confirmedNotice.value = "";
}
/** 状态徽章语义色（token 语义面；rejected 也可见——生命周期语义保留）。 */
function statusTone(status) {
    if (status === "published")
        return "tone-ok";
    if (status === "draft")
        return "tone-warn";
    if (status === "rejected")
        return "tone-bad";
    return "tone-muted";
}
/** 生命周期徽章（PC）。 */
function lifecycleTone(status) {
    if (status === "active")
        return "tone-ok";
    if (status === "draft" || status === "identity")
        return "tone-warn";
    if (status === "deprecated" || status === "superseded")
        return "tone-muted";
    return "tone-muted";
}
function formatTime(value) {
    if (!value)
        return "—";
    return value.slice(0, 16).replace("T", " ");
}
const latestDraft = computed(() => {
    if (!detail.value)
        return null;
    return detail.value.revisions.find(r => r.status === "draft") ?? null;
});
onMounted(loadLists);
watch(currentProjectId, () => {
    closeDetail();
    loadLists();
});
const __VLS_ctx = {};
let __VLS_components;
let __VLS_intrinsics;
let __VLS_directives;
void __VLS_ctx, __VLS_components, __VLS_intrinsics, __VLS_directives;
// @ts-ignore
__VLS_withDotValue(activeTab, {});
// @ts-ignore
__VLS_withDotValue(kbRows, {});
// @ts-ignore
__VLS_withDotValue(pcRows, {});
// @ts-ignore
__VLS_withDotValue(errorText, {});
// @ts-ignore
__VLS_withDotValue(loading, {});
// @ts-ignore
__VLS_withDotValue(detail, {});
// @ts-ignore
__VLS_withDotValue(detailLoading, {});
// @ts-ignore
__VLS_withDotValue(confirmedNotice, {});
// @ts-ignore
__VLS_withDotValue(latestDraft, {});
// @ts-ignore
__VLS_withDotValue(confirming, {});
void {};
/** @type {__VLS_StyleScopedClasses['knowledge-tab']} */ ;
/** @type {__VLS_StyleScopedClasses['knowledge-error']} */ ;
/** @type {__VLS_StyleScopedClasses['kb-row']} */ ;
/** @type {__VLS_StyleScopedClasses['kb-confirm']} */ ;
/** @type {__VLS_StyleScopedClasses['kb-revisions']} */ ;
/** @type {__VLS_StyleScopedClasses['pc-row']} */ ;
/** @type {__VLS_StyleScopedClasses['pc-excerpt']} */ ;
const __VLS_0 = PageFrame || PageFrame;
// @ts-ignore
const __VLS_1 = __VLS_asFunctionalComponent1(__VLS_0, new __VLS_0({
    // @ts-ignore
    title: (__VLS_unwrap(t, {})('s01.knowledge.page.title')), description: (__VLS_unwrap(t, {})('s01.knowledge.page.description')),
}));
const __VLS_2 = __VLS_1({
    title: (__VLS_unwrap(t, {})('s01.knowledge.page.title')),
    description: (__VLS_unwrap(t, {})('s01.knowledge.page.description')),
}, ...__VLS_functionalComponentArgsRest(__VLS_1));
const { default: __VLS_5 } = __VLS_nonNull(__VLS_3.slots);
__VLS_asFunctionalElement1(__VLS_intrinsics.div, __VLS_intrinsics.div)({
    ...{ class: "knowledge-page" },
});
/** @type {__VLS_StyleScopedClasses['knowledge-page']} */ ;
__VLS_asFunctionalElement1(__VLS_intrinsics.div, __VLS_intrinsics.div)({
    ...{ class: "knowledge-tabs" },
    role: "tablist",
});
/** @type {__VLS_StyleScopedClasses['knowledge-tabs']} */ ;
__VLS_asFunctionalElement1(__VLS_intrinsics.button, __VLS_intrinsics.button)({
    ...{ onClick: // @ts-ignore
        (...[$event]) => {
            void $event;
            return (activeTab.value = 'kb');
            // @ts-ignore
            [t, t, activeTab,];
        } },
    role: "tab",
    'aria-selected': (activeTab.value === 'kb'),
    ...{ class: "knowledge-tab" },
    ...{ class: ({ 'is-active': activeTab.value === 'kb' }) },
});
/** @type {__VLS_StyleScopedClasses['knowledge-tab']} */ ;
/** @type {__VLS_StyleScopedClasses['is-active']} */ ;
(__VLS_unwrap(t, {})("s01.knowledge.tab.kb"));
(kbRows.value.length);
__VLS_asFunctionalElement1(__VLS_intrinsics.button, __VLS_intrinsics.button)({
    ...{ onClick: // @ts-ignore
        (...[$event]) => {
            void $event;
            return (activeTab.value = 'pc');
            // @ts-ignore
            [t, activeTab, activeTab, activeTab, kbRows,];
        } },
    role: "tab",
    'aria-selected': (activeTab.value === 'pc'),
    ...{ class: "knowledge-tab" },
    ...{ class: ({ 'is-active': activeTab.value === 'pc' }) },
});
/** @type {__VLS_StyleScopedClasses['knowledge-tab']} */ ;
/** @type {__VLS_StyleScopedClasses['is-active']} */ ;
(__VLS_unwrap(t, {})("s01.knowledge.tab.pc"));
(pcRows.value.length);
if (errorText.value) {
    __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
        ...{ class: "knowledge-error" },
        role: "alert",
    });
    /** @type {__VLS_StyleScopedClasses['knowledge-error']} */ ;
    (errorText.value);
}
if (loading.value) {
    __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
        ...{ class: "knowledge-hint" },
    });
    /** @type {__VLS_StyleScopedClasses['knowledge-hint']} */ ;
    (__VLS_unwrap(t, {})("s01.knowledge.hint.loading"));
}
else if (!loading.value && !kbRows.value.length && activeTab.value === 'kb') {
    __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
        ...{ class: "knowledge-hint" },
    });
    /** @type {__VLS_StyleScopedClasses['knowledge-hint']} */ ;
    (__VLS_unwrap(t, {})("s01.knowledge.hint.emptyKb"));
}
else if (!loading.value && !pcRows.value.length && activeTab.value === 'pc') {
    __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
        ...{ class: "knowledge-hint" },
    });
    /** @type {__VLS_StyleScopedClasses['knowledge-hint']} */ ;
    (__VLS_unwrap(t, {})("s01.knowledge.hint.emptyPc"));
}
if (activeTab.value === 'kb' && kbRows.value.length) {
    __VLS_asFunctionalElement1(__VLS_intrinsics.ul, __VLS_intrinsics.ul)({
        ...{ class: "kb-list" },
    });
    /** @type {__VLS_StyleScopedClasses['kb-list']} */ ;
    const __VLS_6 = __VLS_tryAsConstant((kbRows.value));
    for (const [row] of __VLS_vFor(__VLS_nonNull(__VLS_6))) {
        __VLS_asFunctionalElement1(__VLS_intrinsics.li, __VLS_intrinsics.li)({
            key: (row.document_id),
            ...{ class: "kb-row" },
            ...{ class: ({
                    'is-open': detail.value?.document.document_id === row.document_id
                }) },
        });
        /** @type {__VLS_StyleScopedClasses['kb-row']} */ ;
        /** @type {__VLS_StyleScopedClasses['is-open']} */ ;
        __VLS_asFunctionalElement1(__VLS_intrinsics.button, __VLS_intrinsics.button)({
            ...{ onClick: // @ts-ignore
                (...[$event]) => {
                    void $event;
                    if (!(activeTab.value === 'kb' && kbRows.value.length))
                        throw 0;
                    return (openDetail(row));
                    // @ts-ignore
                    [t, t, t, t, activeTab, activeTab, activeTab, activeTab, activeTab, kbRows, kbRows, kbRows, pcRows, pcRows, errorText, errorText, loading, loading, loading, detail,];
                } },
            ...{ class: "kb-row-head" },
        });
        /** @type {__VLS_StyleScopedClasses['kb-row-head']} */ ;
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "kb-doc-key" },
        });
        /** @type {__VLS_StyleScopedClasses['kb-doc-key']} */ ;
        (row.doc_key);
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "kb-badge" },
            ...{ class: (statusTone(row.status)) },
        });
        /** @type {__VLS_StyleScopedClasses['kb-badge']} */ ;
        (__VLS_unwrap(t, {})(`s01.knowledge.status.${row.status}`));
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "kb-meta" },
        });
        /** @type {__VLS_StyleScopedClasses['kb-meta']} */ ;
        (__VLS_unwrap(t, {})("s01.knowledge.field.revision"));
        (row.revision_no);
        (formatTime(row.published_at ?? row.updated_at));
        if (detail.value?.document.document_id === row.document_id) {
            __VLS_asFunctionalElement1(__VLS_intrinsics.div, __VLS_intrinsics.div)({
                ...{ class: "kb-detail" },
            });
            /** @type {__VLS_StyleScopedClasses['kb-detail']} */ ;
            if (detailLoading.value) {
                __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
                    ...{ class: "knowledge-hint" },
                });
                /** @type {__VLS_StyleScopedClasses['knowledge-hint']} */ ;
                (__VLS_unwrap(t, {})("s01.knowledge.hint.loading"));
            }
            else {
                if (confirmedNotice.value) {
                    __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
                        ...{ class: "kb-notice" },
                        role: "status",
                    });
                    /** @type {__VLS_StyleScopedClasses['kb-notice']} */ ;
                    (confirmedNotice.value);
                }
                __VLS_asFunctionalElement1(__VLS_intrinsics.div, __VLS_intrinsics.div)({
                    ...{ class: "kb-detail-actions" },
                });
                /** @type {__VLS_StyleScopedClasses['kb-detail-actions']} */ ;
                if (latestDraft.value) {
                    __VLS_asFunctionalElement1(__VLS_intrinsics.button, __VLS_intrinsics.button)({
                        ...{ onClick: // @ts-ignore
                            (...[$event]) => {
                                void $event;
                                if (!(activeTab.value === 'kb' && kbRows.value.length))
                                    throw 0;
                                if (!(detail.value?.document.document_id === row.document_id))
                                    throw 0;
                                if (!!(detailLoading.value))
                                    throw 0;
                                if (!(latestDraft.value))
                                    throw 0;
                                return (confirmPublish(row));
                                // @ts-ignore
                                [t, t, t, detail, detailLoading, confirmedNotice, confirmedNotice, latestDraft,];
                            } },
                        ...{ class: "kb-confirm" },
                        disabled: (confirming.value),
                    });
                    /** @type {__VLS_StyleScopedClasses['kb-confirm']} */ ;
                    (confirming.value
                        ? __VLS_unwrap(t, {})("s01.knowledge.action.confirming")
                        : __VLS_unwrap(t, {})("s01.knowledge.action.confirm"));
                }
                __VLS_asFunctionalElement1(__VLS_intrinsics.button, __VLS_intrinsics.button)({
                    ...{ onClick: (closeDetail) },
                    ...{ class: "kb-close" },
                });
                /** @type {__VLS_StyleScopedClasses['kb-close']} */ ;
                (__VLS_unwrap(t, {})("s01.knowledge.action.close"));
                __VLS_asFunctionalElement1(__VLS_intrinsics.details, __VLS_intrinsics.details)({
                    ...{ class: "kb-revisions" },
                    open: true,
                });
                /** @type {__VLS_StyleScopedClasses['kb-revisions']} */ ;
                __VLS_asFunctionalElement1(__VLS_intrinsics.summary, __VLS_intrinsics.summary)({});
                (__VLS_unwrap(t, {})("s01.knowledge.field.revisions"));
                (detail.value.revisions.length);
                __VLS_asFunctionalElement1(__VLS_intrinsics.ul, __VLS_intrinsics.ul)({});
                const __VLS_7 = __VLS_tryAsConstant((detail.value.revisions));
                for (const [rev] of __VLS_vFor(__VLS_nonNull(__VLS_7))) {
                    __VLS_asFunctionalElement1(__VLS_intrinsics.li, __VLS_intrinsics.li)({
                        key: (rev.revision_id),
                    });
                    __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
                        ...{ class: "kb-badge" },
                        ...{ class: (statusTone(rev.status)) },
                    });
                    /** @type {__VLS_StyleScopedClasses['kb-badge']} */ ;
                    (__VLS_unwrap(t, {})(`s01.knowledge.status.${rev.status}`));
                    (__VLS_unwrap(t, {})("s01.knowledge.field.revision"));
                    (rev.revision_no);
                    (formatTime(rev.published_at ?? rev.created_at));
                    // @ts-ignore
                    [t, t, t, t, t, t, detail, detail, confirming, confirming,];
                }
                __VLS_asFunctionalElement1(__VLS_intrinsics.details, __VLS_intrinsics.details)({
                    ...{ class: "kb-content" },
                    open: true,
                });
                /** @type {__VLS_StyleScopedClasses['kb-content']} */ ;
                __VLS_asFunctionalElement1(__VLS_intrinsics.summary, __VLS_intrinsics.summary)({});
                (__VLS_unwrap(t, {})("s01.knowledge.field.content"));
                const __VLS_8 = MarkdownViewer;
                // @ts-ignore
                const __VLS_9 = __VLS_asFunctionalComponent1(__VLS_8, new __VLS_8({
                    // @ts-ignore
                    modelValue: (detail.value.content ?? ''),
                }));
                const __VLS_10 = __VLS_9({
                    modelValue: (detail.value.content ?? ''),
                }, ...__VLS_functionalComponentArgsRest(__VLS_9));
            }
        }
        // @ts-ignore
        [t, detail,];
    }
}
if (activeTab.value === 'pc' && pcRows.value.length) {
    __VLS_asFunctionalElement1(__VLS_intrinsics.ul, __VLS_intrinsics.ul)({
        ...{ class: "pc-list" },
    });
    /** @type {__VLS_StyleScopedClasses['pc-list']} */ ;
    const __VLS_13 = __VLS_tryAsConstant((pcRows.value));
    for (const [obj] of __VLS_vFor(__VLS_nonNull(__VLS_13))) {
        __VLS_asFunctionalElement1(__VLS_intrinsics.li, __VLS_intrinsics.li)({
            key: (obj.object_id),
            ...{ class: "pc-row" },
        });
        /** @type {__VLS_StyleScopedClasses['pc-row']} */ ;
        __VLS_asFunctionalElement1(__VLS_intrinsics.div, __VLS_intrinsics.div)({
            ...{ class: "pc-head" },
        });
        /** @type {__VLS_StyleScopedClasses['pc-head']} */ ;
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "pc-name" },
        });
        /** @type {__VLS_StyleScopedClasses['pc-name']} */ ;
        (obj.display_name);
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "pc-type" },
        });
        /** @type {__VLS_StyleScopedClasses['pc-type']} */ ;
        (obj.object_type);
        __VLS_asFunctionalElement1(__VLS_intrinsics.span, __VLS_intrinsics.span)({
            ...{ class: "kb-badge" },
            ...{ class: (lifecycleTone(obj.lifecycle_status)) },
        });
        /** @type {__VLS_StyleScopedClasses['kb-badge']} */ ;
        (__VLS_unwrap(t, {})(`s01.knowledge.lifecycle.${obj.lifecycle_status}`));
        if (obj.change_summary) {
            __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
                ...{ class: "pc-summary" },
            });
            /** @type {__VLS_StyleScopedClasses['pc-summary']} */ ;
            (obj.change_summary);
        }
        if (obj.excerpt) {
            __VLS_asFunctionalElement1(__VLS_intrinsics.p, __VLS_intrinsics.p)({
                ...{ class: "pc-excerpt" },
            });
            /** @type {__VLS_StyleScopedClasses['pc-excerpt']} */ ;
            (obj.excerpt);
        }
        // @ts-ignore
        [t, activeTab, pcRows, pcRows,];
    }
}
// @ts-ignore
[];
var __VLS_3;
// @ts-ignore
[];
const __VLS_export = (await import('vue')).defineComponent({});
export default {};
//# sourceMappingURL=KnowledgeView.vue.js.map