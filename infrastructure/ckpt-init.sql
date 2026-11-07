-- Checkpoint-1 专用 PG 的初始化脚本（docker-entrypoint-initdb.d，仅空卷首启执行）：
-- POSTGRES_DB=pm_ckpt 已由镜像直建；此处补 lmms_ckpt（lmms 全家经
-- DATABASE_URL=...lmms_ckpt 指入）。
CREATE DATABASE lmms_ckpt;
