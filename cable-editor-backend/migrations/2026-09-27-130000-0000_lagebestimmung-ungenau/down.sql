-- Only the default: which ungenau were set explicitly can't be told apart
alter table schacht
    alter column lagebestimmung set default 'unbekannt';
alter table trasse
    alter column lagebestimmung set default 'unbekannt';
