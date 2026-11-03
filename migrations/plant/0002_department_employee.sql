-- 工廠層（docs/adr/0007-multi-plant.md）：部門與員工，每個工廠 schema 各一份

-- 部門（TipTop: gem_file，aooi030；specs/features/aoo/department_maintenance.feature）
CREATE TABLE department (
    dept_code        varchar(10) PRIMARY KEY,
    name             varchar(80) NOT NULL,
    full_name        varchar(80),
    accounting_dept  boolean NOT NULL DEFAULT true,
    expense_type     char(1) CHECK (expense_type IN ('M', 'P', 'R', 'S')),
    control_type     char(1) NOT NULL DEFAULT '3' CHECK (control_type IN ('1', '2', '3')),
    cost_center      varchar(10) REFERENCES department (dept_code),
    active           boolean NOT NULL DEFAULT true,
    created_by       text NOT NULL,
    created_dept     text NOT NULL,
    created_on       date NOT NULL DEFAULT CURRENT_DATE,
    updated_by       text,
    updated_on       date
);
COMMENT ON TABLE department IS 'TipTop: gem_file 部門名稱資料 (aooi030)；工廠層';
COMMENT ON COLUMN department.dept_code IS 'gem01 部門編號（大寫、建立後不可修改）';
COMMENT ON COLUMN department.name IS 'gem02 部門名稱（簡稱）';
COMMENT ON COLUMN department.full_name IS 'gem03 部門全稱';
COMMENT ON COLUMN department.accounting_dept IS 'gem05 是否為會計部門';
COMMENT ON COLUMN department.expense_type IS 'gem07 費用類別：M 管理、P 製造、R 研發、S 銷售';
COMMENT ON COLUMN department.control_type IS 'gem09 管理類別：1 成本中心、2 利潤中心、3 其他';
COMMENT ON COLUMN department.cost_center IS 'gem10 對應成本中心';
COMMENT ON COLUMN department.active IS 'gemacti 資料有效碼';
COMMENT ON COLUMN department.created_by IS 'gemuser 資料所有者';
COMMENT ON COLUMN department.created_dept IS 'gemgrup 資料所有部門';
COMMENT ON COLUMN department.updated_by IS 'gemmodu 資料修改者';
COMMENT ON COLUMN department.updated_on IS 'gemdate 最近修改日';

-- 員工（TipTop: gen_file，aooi040；specs/features/aoo/employee_maintenance.feature）
CREATE TABLE employee (
    employee_id   varchar(10) PRIMARY KEY,
    name          varchar(40) NOT NULL,
    dept_code     varchar(10) REFERENCES department (dept_code),
    title         varchar(80),
    extension     varchar(5),
    email         varchar(80),
    active        boolean NOT NULL DEFAULT true,
    created_by    text NOT NULL,
    created_dept  text NOT NULL,
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date
);
COMMENT ON TABLE employee IS 'TipTop: gen_file 員工姓名資料 (aooi040)；工廠層';
COMMENT ON COLUMN employee.employee_id IS 'gen01 員工編號（大寫、建立後不可修改）';
COMMENT ON COLUMN employee.name IS 'gen02 員工姓名';
COMMENT ON COLUMN employee.dept_code IS 'gen03 所屬部門編號';
COMMENT ON COLUMN employee.title IS 'gen04 職稱';
COMMENT ON COLUMN employee.extension IS 'gen05 分機';
COMMENT ON COLUMN employee.email IS 'gen06 E-mail';
COMMENT ON COLUMN employee.active IS 'genacti 資料有效碼';
COMMENT ON COLUMN employee.created_by IS 'genuser 資料所有者';
COMMENT ON COLUMN employee.created_dept IS 'gengrup 資料所有部門';
COMMENT ON COLUMN employee.updated_by IS 'genmodu 資料修改者';
COMMENT ON COLUMN employee.updated_on IS 'gendate 最近修改日';
