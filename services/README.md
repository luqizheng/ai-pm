# services/ —— 外部服务的配置挂载点（无源码）

本目录下的 `token-service/` 与 `notification-service/` **只持有运行配置**
（`config/config.yaml`，供 full-dev compose bind-mount），源码与镜像构建
在外部 lmms 仓库（`IMAGE_*` 变量经 `.image.env` 加载预构建镜像）。

历史遗留：`docker-compose-services.yml` 曾引用本目录下的 Dockerfile，
但文件不存在（本地 build 路径是坏占位）——批 G 已移除 build 段。
