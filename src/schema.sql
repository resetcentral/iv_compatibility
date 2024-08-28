CREATE DATABASE IF NOT EXISTS iv_compatibility;

USE iv_compatibility;

CREATE TABLE IF NOT EXISTS infusion_type (
    id TINYINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    type VARCHAR(255) NOT NULL
);

CREATE TABLE IF NOT EXISTS infusion (
    id INT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL UNIQUE,
    type TINYINT UNSIGNED NOT NULL,
    FOREIGN KEY (type) REFERENCES infusion_type(id)
);

CREATE TABLE IF NOT EXISTS infusion_compatibility (
    infusion_a INT UNSIGNED NOT NULL,
    infusion_b INT UNSIGNED NOT NULL,
    compatible_results TINYINT UNSIGNED NOT NULL,
    incompatible_results TINYINT UNSIGNED NOT NULL,
    mixed_results TINYINT UNSIGNED NOT NULL,
    PRIMARY KEY (infusion_a, infusion_b),
    FOREIGN KEY (infusion_a) REFERENCES infusion(id),
    FOREIGN KEY (infusion_b) REFERENCES infusion(id)
);