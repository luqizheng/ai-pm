# -*- coding: utf-8 -*-
# Board04 v2：对 C5 提交态 SideNav.vue 一次性重放 DS 化改造（执行后删除）
import io

path = "src/components/shell/SideNav.vue"
with io.open(path, encoding="utf-8") as f:
    text = f.read()

repls = []

# 1) imports：ProjectSwitcher + DS 组件；去掉 Chevron 两组与 navGroups
repls.append((
    'import IconTooltip from "./IconTooltip.vue";',
    'import { NavigationGroup } from "@/components/ui/navigation-group";\n'
    'import { SidebarItem } from "@/components/ui/sidebar-item";\n'
    'import IconTooltip from "./IconTooltip.vue";\n'
    'import ProjectSwitcher from "./ProjectSwitcher.vue";'
))
repls.append((
    "import {\n"
    "  ChevronDown,\n"
    "  ChevronRight,\n"
    "  Keyboard,\n"
    "  PanelLeftClose,\n"
    "  PanelLeftOpen\n"
    '} from "lucide-vue-next";',
    "import {\n"
    "  Keyboard,\n"
    "  PanelLeftClose,\n"
    "  PanelLeftOpen\n"
    '} from "lucide-vue-next";'
))
# 2) toggleGroup → setGroupCollapsed
repls.append((
    "function toggleGroup(group: NavGroup) {\n"
    "  collapsedGroups.value = {\n"
    "    ...collapsedGroups.value,\n"
    "    [group.id]: !isGroupCollapsed(group)\n"
    "  };\n"
    "}",
    "function setGroupCollapsed(group: NavGroup, value: boolean) {\n"
    "  collapsedGroups.value = { ...collapsedGroups.value, [group.id]: value };\n"
    "}"
))

# 3) REGION 映射补项目区五组
repls.append((
    '  projectHome: "project",\n'
    '  modules: "project",\n'
    '  projectSettings: "project",',
    '  projectHome: "project",\n'
    '  workItems: "project",\n'
    '  development: "project",\n'
    '  knowledge: "project",\n'
    '  review: "project",\n'
    '  projectSettings: "project",'
))

# 4) itemBadge/hasUnreadDot/itemLabel → itemBadgeProps（Board 04 徽标语言）
repls.append((
    "/**\n"
    " * 动态徽标数：人工审核中心 = 项目级聚合待处理数（R09，与指挥中心面板同源）。\n"
    " * 聚合不可用或为 0 时不显示——不伪造数字（V1.1 §6.3）。\n"
    " */\n"
    "function itemBadge(item: NavItem): number | undefined {\n"
    "  if (item.path === REVIEW_CENTER_PATH) {\n"
    "    const total = reviewCenter.totalCount;\n"
    "    return total !== null && total > 0 ? total : undefined;\n"
    "  }\n"
    "  return item.badge;\n"
    "}\n"
    "\n"
    "/** 该项是否带未读圆点（Figma：收件箱未读、AI 执行中心未读） */\n"
    "function hasUnreadDot(item: NavItem) {\n"
    "  const badge = itemBadge(item);\n"
    "  return badge !== undefined && badge > 0;\n"
    "}\n"
    "\n"
    "/** 折叠态下 Tooltip 的文案（图标按钮的可访问名称） */\n"
    "function itemLabel(item: NavItem) {\n"
    "  return t(item.labelKey);\n"
    "}",
    "/**\n"
    " * 徽标 props（Board 04 徽标语言）：人工审核中心动态计数（R09，聚合不可用\n"
    " * 或为 0 时不显示——不伪造数字）；PLANNED 项回落 status 徽标（规划）。\n"
    " */\n"
    "function itemBadgeProps(item: NavItem): {\n"
    "  badge?: string | number;\n"
    '  badgeType: "count" | "status";\n'
    "} {\n"
    "  if (item.path === REVIEW_CENTER_PATH) {\n"
    "    const total = reviewCenter.totalCount;\n"
    "    return total !== null && total > 0\n"
    '      ? { badge: total, badgeType: "count" as const }\n'
    '      : { badgeType: "count" as const };\n'
    "  }\n"
    "  if (item.planned)\n"
    "    return {\n"
    '      badge: t("shell.app.side.plannedBadge"),\n'
    '      badgeType: "status" as const\n'
    "    };\n"
    '  if (item.badge !== undefined)\n'
    '    return { badge: item.badge, badgeType: "count" as const };\n'
    '  return { badgeType: "count" as const };\n'
    "}"
))

# 5) isActive：子路由前缀兜底（devenv 下 Runners 跨父前缀）
repls.append((
    "function isActive(item: NavItem) {\n"
    "  return isNavItemActive(item, currentPath.value);\n"
    "}",
    "function isActive(item: NavItem) {\n"
    "  if (isNavItemActive(item, currentPath.value)) return true;\n"
    "  // 子路由前缀兜底：跨父前缀的子项（如 devenv 下的 Runners\n"
    "  // /project/settings/runners）命中时父项同样激活，保证子菜单可见。\n"
    "  return (\n"
    "    item.children?.some(\n"
    "      child =>\n"
    "        currentPath.value === child.path ||\n"
    "        currentPath.value.startsWith(`${child.path}/`)\n"
    "    ) ?? false\n"
    "  );\n"
    "}"
))

# 6) 模板整体替换：groups div → 底部键盘提示注释行（整行锚点）
start_marker = '    <div class="shell-sidenav__groups">'
end_marker = "    <!-- 底部键盘提示（展开态带文案与 ? 芯片；折叠态仅图标） -->\n"

new_inner = (
    '    <div class="shell-sidenav__groups">\n'
    "      <!-- 折叠态：纯图标直列（DS SidebarItem collapsed；组标题隐藏，区域分隔线保留） -->\n"
    "      <template v-if=\"shell.sidebarCollapsed\">\n"
    '        <template v-for="(group, index) in renderedGroups" :key="group.id">\n'
    "          <div\n"
    '            v-if="showDivider(group, index)"\n'
    '            class="shell-sidenav__divider"\n'
    '            aria-hidden="true"\n'
    "          />\n"
    "          <div\n"
    "            v-if=\"group.id === 'projectHome'\"\n"
    '            class="shell-sidenav__project"\n'
    "          >\n"
    "            <ProjectSwitcher\n"
    '              variant="header"\n'
    "                searchable\n"
    '                :projects="shell.projects"\n'
    '                :current-id="shell.currentProjectId"\n'
    '                :switching-to-id="shell.switchingProjectId"\n'
    '                @select="onSelectProject"\n'
    "            />\n"
    "          </div>\n"
    "          <template v-for=\"item in group.items\" :key=\"item.path\">\n"
    "            <SidebarItem\n"
    '              v-if="item.icon"\n'
    "              collapsed\n"
    '              :label="t(item.labelKey)"\n'
    '              :icon="item.icon"\n'
    '              :to="item.path"\n'
    "              :state=\"isActive(item) ? 'active' : 'default'\"\n"
    "            />\n"
    "          </template>\n"
    "        </template>\n"
    "      </template>\n"
    "\n"
    "      <!-- 展开态：DS NavigationGroup + SidebarItem（Board 03/04/05 版式） -->\n"
    "      <template v-else>\n"
    '        <template v-for="(group, index) in renderedGroups" :key="group.id">\n'
    "          <div\n"
    '            v-if="showDivider(group, index)"\n'
    '            class="shell-sidenav__divider"\n'
    '            aria-hidden="true"\n'
    "          />\n"
    "          <div\n"
    "            v-if=\"group.id === 'projectHome'\"\n"
    '            class="shell-sidenav__project"\n'
    "          >\n"
    "            <ProjectSwitcher\n"
    '              variant="header"\n'
    "              searchable\n"
    '              :projects="shell.projects"\n'
    '              :current-id="shell.currentProjectId"\n'
    '              :switching-to-id="shell.switchingProjectId"\n'
    '              @select="onSelectProject"\n'
    "            />\n"
    "          </div>\n"
    "          <NavigationGroup\n"
    '            :title="t(group.labelKey)"\n'
    '            :badge="group.badge"\n'
    '            :collapsed="isGroupCollapsed(group)"\n'
    '            @update:collapsed="value => setGroupCollapsed(group, value)"\n'
    "          >\n"
    "            <!-- children 感知通用分支：带子项的条目渲染为嵌套组\n"
    "                 （团队设置四子项；项目设置的开发环境三子项） -->\n"
    '            <template v-for="item in group.items" :key="item.path">\n'
    "              <NavigationGroup\n"
    '                v-if="item.children?.length"\n'
    '                :title="t(item.labelKey)"\n'
    "              >\n"
    "                <SidebarItem\n"
    '                  v-for="child in item.children"\n'
    '                  :key="child.path"\n'
    '                  :label="t(child.labelKey)"\n'
    '                  :to="child.path"\n'
    '                  :state="\n'
    "                    isNavChildActive(child, currentPath) ? 'active' : 'default'\n"
    '                  "\n'
    '                  :badge="\n'
    "                    child.planned\n"
    "                      ? t('shell.app.side.plannedBadge')\n"
    "                      : undefined\n"
    '                  "\n'
    '                  badge-type="status"\n'
    "                />\n"
    "              </NavigationGroup>\n"
    "              <SidebarItem\n"
    "                v-else\n"
    '                :label="t(item.labelKey)"\n'
    '                :icon="item.icon"\n'
    '                :to="item.path"\n'
    "                :state=\"isActive(item) ? 'active' : 'default'\"\n"
    '                v-bind="itemBadgeProps(item)"\n'
    "              />\n"
    "            </template>\n"
    "          </NavigationGroup>\n"
    "        </template>\n"
    "      </template>\n"
    "    </div>\n"
    "\n"
    "    <!-- 底部键盘提示（? 键位芯片；展开带文案/折叠仅图标） -->\n"
)

start = text.index(start_marker)
end = text.index(end_marker) + len(end_marker)
text = text[:start] + new_inner + text[end:]

for old, new in repls:
    count = text.count(old)
    assert count == 1, ("MISS", old[:50].replace("\n", "\\n"), count)
    text = text.replace(old, new, 1)

with io.open(path, "w", encoding="utf-8", newline="") as f:
    f.write(text)
print("SideNav.vue Board04 v2 ok,", len(repls), "script replacements + template/css splice")
