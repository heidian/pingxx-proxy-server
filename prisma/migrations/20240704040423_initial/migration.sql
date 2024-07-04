-- CreateTable
CREATE TABLE `App` (
    `id` VARCHAR(191) NOT NULL,
    `name` VARCHAR(191) NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `SubApp` (
    `id` VARCHAR(191) NOT NULL,
    `appId` VARCHAR(191) NOT NULL,
    `name` VARCHAR(191) NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `ChannelParams` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `appId` VARCHAR(191) NULL,
    `subAppId` VARCHAR(191) NULL,
    `channel` VARCHAR(191) NOT NULL,
    `params` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    UNIQUE INDEX `ChannelParams_appId_subAppId_channel_key`(`appId`, `subAppId`, `channel`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `Order` (
    `id` VARCHAR(191) NOT NULL,
    `appId` VARCHAR(191) NOT NULL,
    `subAppId` VARCHAR(191) NOT NULL,
    `uid` VARCHAR(191) NOT NULL,
    `merchantOrderNo` VARCHAR(191) NOT NULL,
    `status` VARCHAR(191) NOT NULL,
    `paid` BOOLEAN NOT NULL,
    `refunded` BOOLEAN NOT NULL,
    `amount` INTEGER NOT NULL,
    `amountPaid` INTEGER NOT NULL,
    `amountRefunded` INTEGER NOT NULL,
    `clientIp` VARCHAR(191) NOT NULL,
    `subject` VARCHAR(191) NOT NULL,
    `body` VARCHAR(191) NOT NULL,
    `currency` VARCHAR(191) NOT NULL,
    `timePaid` INTEGER NULL,
    `timeExpire` INTEGER NOT NULL,
    `metadata` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `Charge` (
    `id` VARCHAR(191) NOT NULL,
    `appId` VARCHAR(191) NOT NULL,
    `orderId` VARCHAR(191) NULL,
    `channel` VARCHAR(191) NOT NULL,
    `merchantOrderNo` VARCHAR(191) NOT NULL,
    `paid` BOOLEAN NOT NULL,
    `amount` INTEGER NOT NULL,
    `clientIp` VARCHAR(191) NOT NULL,
    `subject` VARCHAR(191) NOT NULL,
    `body` VARCHAR(191) NOT NULL,
    `currency` VARCHAR(191) NOT NULL,
    `extra` JSON NOT NULL,
    `credential` JSON NOT NULL,
    `timePaid` INTEGER NULL,
    `timeExpire` INTEGER NOT NULL,
    `failureCode` VARCHAR(191) NULL,
    `failureMsg` TEXT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `Refund` (
    `id` VARCHAR(191) NOT NULL,
    `appId` VARCHAR(191) NOT NULL,
    `chargeId` VARCHAR(191) NOT NULL,
    `orderId` VARCHAR(191) NULL,
    `merchantOrderNo` VARCHAR(191) NOT NULL,
    `status` VARCHAR(191) NOT NULL,
    `amount` INTEGER NOT NULL,
    `description` VARCHAR(191) NOT NULL,
    `extra` JSON NOT NULL,
    `timeSucceed` INTEGER NULL,
    `failureCode` VARCHAR(191) NULL,
    `failureMsg` TEXT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `ChargeNotifyHistory` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `chargeId` VARCHAR(191) NOT NULL,
    `refundId` VARCHAR(191) NULL,
    `data` TEXT NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    INDEX `ChargeNotifyHistory_chargeId_idx`(`chargeId`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- CreateTable
CREATE TABLE `AppWebhookConfig` (
    `id` INTEGER NOT NULL AUTO_INCREMENT,
    `appId` VARCHAR(191) NOT NULL,
    `endpoint` VARCHAR(191) NOT NULL,
    `events` JSON NOT NULL,
    `createdAt` DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    `updatedAt` DATETIME(3) NOT NULL,

    UNIQUE INDEX `AppWebhookConfig_appId_endpoint_key`(`appId`, `endpoint`),
    PRIMARY KEY (`id`)
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- AddForeignKey
ALTER TABLE `SubApp` ADD CONSTRAINT `SubApp_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `ChannelParams` ADD CONSTRAINT `ChannelParams_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `ChannelParams` ADD CONSTRAINT `ChannelParams_subAppId_fkey` FOREIGN KEY (`subAppId`) REFERENCES `SubApp`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Order` ADD CONSTRAINT `Order_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Order` ADD CONSTRAINT `Order_subAppId_fkey` FOREIGN KEY (`subAppId`) REFERENCES `SubApp`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Charge` ADD CONSTRAINT `Charge_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Charge` ADD CONSTRAINT `Charge_orderId_fkey` FOREIGN KEY (`orderId`) REFERENCES `Order`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Refund` ADD CONSTRAINT `Refund_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Refund` ADD CONSTRAINT `Refund_chargeId_fkey` FOREIGN KEY (`chargeId`) REFERENCES `Charge`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `Refund` ADD CONSTRAINT `Refund_orderId_fkey` FOREIGN KEY (`orderId`) REFERENCES `Order`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE `AppWebhookConfig` ADD CONSTRAINT `AppWebhookConfig_appId_fkey` FOREIGN KEY (`appId`) REFERENCES `App`(`id`) ON DELETE CASCADE ON UPDATE CASCADE;
