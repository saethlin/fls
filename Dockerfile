FROM centos:6

RUN yum update -y && \
    yum install -y curl gcc && \
    yum clean all

COPY entrypoint.sh /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
