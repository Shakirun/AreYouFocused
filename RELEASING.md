# Ветки и релизы

## Модель

| Ветка | Назначение | Версия в `package.json` |
|--------|------------|-------------------------|
| **`main`** | Стабильные релизы, только через merge из `develop` (MR) | `0.x.y` без суффикса |
| **`develop`** | Интеграция фич, нестабильная линия | `0.x.y-dev.N` (см. ниже) |
| **`feature/*`** | Ответвления от `develop`, MR обратно в `develop` | не меняют глобальную политику версий |

Фичи **не** пушатся напрямую в `main`. Поток: `feature/...` → **`develop`** → по готовности релиза **`develop` → `main`** одним MR с описанием релиза.

## Ежедневная работа

1. От `develop`: `git fetch origin && git checkout develop && git pull`.
2. Создать ветку: `git checkout -b feature/short-name`.
3. Коммиты по TDD, пуш: `git push -u origin feature/short-name`.
4. Открыть **MR в `develop`**, дождаться ревью/CI, merge.

При необходимости поднять нестабильный счётчик на `develop` после крупных вливок: отдельный коммит `chore: bump dev pre-release` (`0.2.0-dev.0` → `0.2.0-dev.1`).

## Релиз в `main`

Когда на `develop` набрано достаточно для релиза:

1. Убедиться, что `develop` зелёный (тесты, сборка).
2. Создать **MR `develop` → `main`** (не squash всей истории develop в один коммит без нужды — предпочтительно merge commit или squash по политике команды).
3. В описании MR использовать шаблон **«Release → main»** (`.github/PULL_REQUEST_TEMPLATE/release_to_main.md`): перечислить **что вошло в релиз**, версию, риски, проверки.
4. В той же ветке релиза (или follow-up сразу после merge в `main`): выставить в `package.json` **стабильную** версию `0.x.y`, тег `v0.x.y` по желанию.
5. После merge в `main`: смержить или ребейзнуть `main` обратно в `develop`, чтобы `develop` не отставала; на `develop` снова поднять dev-версию под следующий цикл (`0.(x+1).0-dev.0`).

## Защита веток на GitHub

В **Settings → Branches → Branch protection rules** (или **Rulesets**):

### `main`

- Require a pull request before merging.
- Require approvals (минимум 1, если работаете не один — настроить под команду).
- **Do not allow bypass** для админов, если нужна строгость.
- Restrict who can push: никто напрямую (только через MR).
- Опционально: required status checks.

### `develop`

- Require a pull request before merging (фичи только через MR).
- Разрешить прямой push только если осознанно нужен «только владелец» — по умолчанию лучше тоже только через MR.
- Запретить force-push и удаление ветки.

Точные переключатели зависят от UI GitHub; при использовании **Rulesets** объедините правила по префиксам `main` и `develop`.

## CLI (опционально)

С `gh` и правами на репозиторий можно настраивать правила через API; для большинства достаточно UI выше.
