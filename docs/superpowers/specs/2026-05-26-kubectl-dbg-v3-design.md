# kubectl-dbg v3.0.0 设计规格

## 版本信息
- **版本**: 3.0.0
- **日期**: 2026-05-26
- **状态**: 已批准

## 概述

kubectl-dbg 是一个 Kubernetes Pod 调试工具，通过 SSH 连接到节点，使用 nsenter 进入 Pod 的 namespace，直接使用宿主机原生工具进行调试。

### 核心功能
1. **AI 智能诊断** - 调用 OpenAI 兼容 API 分析问题
2. **时间旅行调试** - 展示 Pod 生命周期事件
3. **Pod 配置对比** - 对比当前 Pod 与 RS 期望配置

### 设计原则
- **生产安全优先**: 所有操作默认只读，不修改任何资源
- **配置灵活性**: 支持多种 AI API 端点
- **用户体验**: 保持 kubectl 插件风格

---

## 1. AI 智能诊断

### 功能描述
自动收集 Pod 相关数据，调用 LLM 进行智能诊断分析。

### 命令格式
```bash
kubectl dbg <pod> --ai [--ai-model gpt-4] [--ai-endpoint <url>]
```

### 配置优先级
1. CLI 参数: `--ai-key`, `--ai-endpoint`, `--ai-model`
2. 环境变量: `OPENAI_API_KEY`, `OPENAI_BASE_URL`

### 数据收集
- Pod 日志（最近100行）
- Pod 事件（最近24小时）
- 资源状态（CPU/内存请求/限制）
- 网络诊断结果（可选）
- 容器配置

### 提示词设计
```
你是一个 Kubernetes 调试专家。请分析以下 Pod 的诊断数据，找出可能的问题并提供修复建议。

Pod 信息:
- 名称: {pod_name}
- 命名空间: {namespace}
- 节点: {node_name}
- 镜像: {image}
- 状态: {phase}

{diagnostic_data}

请按以下格式输出:
## 诊断结论
{conclusion}

## 可能原因
{causes}

## 修复建议
{fixes}
```

### 错误处理
- API 连接失败: 显示详细错误信息
- API 返回错误: 显示错误码和消息
- 超时: 30秒超时，自动取消

---

## 2. 时间旅行调试

### 功能描述
展示 Pod 生命周期中的关键事件，帮助理解 Pod 历史状态变化。

### 命令格式
```bash
kubectl dbg <pod> --timeline [--since <duration>] [--until <time>]
```

### 时间范围
- `--since`: 默认 24h，可选 1h, 6h, 12h, 24h, 48h, 168h (1周)
- `--until`: 默认 now

### 展示内容

#### Pod 事件时间线
```
=== Pod Timeline (my-pod/default) ===
2026-05-26 10:30:15  ✅ Created
2026-05-26 10:30:16  📍 Scheduled to node-1
2026-05-26 10:30:18  🐳 Pulling image
2026-05-26 10:30:25  ✅ Container started (main)
2026-05-26 10:30:26  ✅ Ready
2026-05-26 14:22:10  ⚠️  Warning: Liveness probe failed
2026-05-26 14:22:40  🔄 Container restarted
```

#### 容器重启历史
```
=== Container Restarts ===
Total restarts: 3
Last restart: 2026-05-26 14:22:40 (2 hours ago)
Restart reasons:
- 2026-05-26 14:22:40: Error (exit code 1)
- 2026-05-26 11:15:22: Error (exit code 1)
- 2026-05-26 08:30:05: OOMKilled
```

#### 状态变化历史
```
=== Status Transitions ===
- 2026-05-26 10:30:15  Pending
- 2026-05-26 10:30:25  Running (Ready)
- 2026-05-26 14:22:40  Running (Restarting)
```

---

## 3. Pod 配置对比

### 功能描述
对比 Pod 实际运行配置与 ReplicaSet 期望配置的差异。

### 命令格式
```bash
kubectl dbg <pod> --diff [--namespace <ns>]
```

### 对比维度

#### 1. 容器配置对比
| 字段 | Pod 实际 | RS 期望 | 差异 |
|------|----------|---------|------|
| image | nginx:1.19 | nginx:1.21 | ⚠️ |
| replicas | - | 3 | - |
| resources.limits.cpu | 500m | 1000m | ⚠️ |
| env.DB_HOST | db-prod | db-staging | ⚠️ |

#### 2. 差异分类
- 🔴 **关键差异**: 可能导致问题的配置差异
- ⚠️ **警告**: 需要关注的差异
- ✅ **一致**: 配置匹配

### 输出格式
```
=== Configuration Diff ===
Namespace: default
Pod: my-pod-7f8d9c6b5-x2p8q
ReplicaSet: my-pod-7f8d9c6b5

🔴 Image Mismatch
   Pod:     nginx:1.19
   RS:      nginx:1.21
   Impact:  可能运行旧版本镜像

⚠️ Resource Limits
   Pod:     cpu: 500m, memory: 512Mi
   RS:      cpu: 1000m, memory: 1Gi
   Impact:  资源限制低于预期，可能导致性能问题

✅ Other settings match
```

---

## 4. CLI 参数规格

### 保留参数（保持现有行为）
| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `<POD>` | | 必填 | 目标 Pod |
| `--namespace` | `-n` | default | 命名空间 |
| `--container` | `-c` | 第一个 | 容器名 |
| `--ssh-key` | `-i` | ~/.ssh/id_rsa | SSH 私钥 |
| `--verbose` | `-v` | false | 详细输出 |
| `--dry-run` | | false | 预览模式 |
| `--diag` | | false | 网络诊断 |
| `--pcap` | | false | 抓包 |
| `--assist` | | false | 交互助手 |
| `--report` | | false | 生成报告 |

### 新增参数
| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `--ai` | | false | 启用 AI 诊断 |
| `--ai-model` | | gpt-4 | AI 模型名称 |
| `--ai-endpoint` | | | AI API 端点 URL |
| `--ai-key` | | | AI API Key |
| `--timeline` | | false | 时间旅行调试 |
| `--since` | | 24h | 时间范围 |
| `--diff` | | false | 配置对比 |
| `--force` | | false | 强制执行高风险操作 |

### 已有参数调整
| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `--ssh-user` | | root | SSH 用户 |
| `--ssh-password` | | | SSH 密码 |
| `--ssh-port` | | 22 | SSH 端口 |
| `--ns-type` | | all | namespace 类型 |
| `--enter-mount` | | false | 进入 mount ns |
| `--runtime` | | auto | 运行时检测 |
| `--kubeconfig` | | auto | kubeconfig |
| `--context` | | 当前 | K8s context |
| `--targets` | | | 诊断目标 |
| `--pcap-filter` | | | BPF 过滤器 |
| `--pcap-count` | | 100 | 抓包数量 |
| `--pcap-output` | | auto | PCAP 输出 |
| `--report-format` | | text | 报告格式 |
| `--report-output` | | | 报告输出 |

---

## 5. 模块设计

### 目录结构
```
src/
├── ai/
│   ├── mod.rs          # AI 模块入口
│   ├── client.rs       # OpenAI 兼容 API 客户端
│   └── analyzer.rs     # 诊断数据分析
├── timeline/
│   ├── mod.rs          # 时间旅行模块入口
│   ├── events.rs       # 事件收集
│   └── formatter.rs    # 时间线格式化
├── diff/
│   ├── mod.rs          # 对比模块入口
│   ├── collector.rs    # 配置收集
│   └── comparator.rs   # 配置对比
└── ...
```

### 新增依赖
```toml
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## 6. 部署脚本更新

### 安装脚本功能
1. 下载对应平台的二进制文件
2. 复制到 `/usr/local/bin`
3. 设置执行权限
4. 创建 kubectl 插件符号链接（可选）
5. 验证安装

### 符号链接创建
```bash
ln -sf /usr/local/bin/kubectl-dbg /usr/local/bin/kubectl-dbg
# 用户可以使用 kubectl dbg <pod> 或 kubectl-dbg <pod>
```

---

## 7. GitHub Actions 更新

### Workflow 调整
1. 更新二进制文件名称为 `kubectl-dbg-*`
2. 更新 artifact 命名
3. 更新 Release 名称

---

## 8. 生产环境安全策略

### 只读原则
- 所有诊断功能默认只读
- 不修改任何 Kubernetes 资源
- 不修改节点配置

### 风险控制
- 潜在修改操作需要 `--force` 参数
- 高风险操作前显示警告
- 保持审计追踪

---

## 9. 验收标准

### AI 智能诊断
- [ ] 支持 OpenAI 兼容 API
- [ ] 支持自定义端点和模型
- [ ] 自动收集诊断数据
- [ ] 格式化输出诊断结果
- [ ] 优雅处理 API 错误

### 时间旅行调试
- [ ] 展示 Pod 事件时间线
- [ ] 显示容器重启历史
- [ ] 支持 `--since` 时间范围
- [ ] 美化输出格式

### Pod 配置对比
- [ ] 获取 ReplicaSet 配置
- [ ] 对比镜像版本
- [ ] 对比资源配置
- [ ] 对比环境变量
- [ ] 高亮差异项

### 整体
- [ ] 所有新参数有完整帮助文档
- [ ] 保留已有功能的兼容性
- [ ] 安装脚本正常工作
- [ ] GitHub Actions 构建成功
- [ ] 文档完整清晰
